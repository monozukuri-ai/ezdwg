"""Write-only drawings that hand entities to the native writer as they are added.

A :class:`StreamingDrawing` offers the tables and ``add_*`` methods of
:class:`~ezdwg.drawing.Drawing` but keeps no entity: each one is validated,
normalised, encoded and queued, and every :data:`~ezdwg.drawing.WRITE_BATCH`
entities the queue becomes object records inside :class:`ezdwg.raw.DwgWriter`.
Memory stays at a few megabytes of Python objects plus about a hundred bytes
per entity in the writer, whatever the drawing size. In exchange, entities
cannot be queried, updated or deleted, and the file is written once by
``save``.

Set ``header["codepage"]`` (AC1015) before the first entity: the code page is
fixed when the first entity reaches the writer. Tables may still grow
afterwards (layers, linetypes, text styles, blocks and image definitions are
written last), and ``set_paper`` may follow the paper-space entities.
"""

from __future__ import annotations

from collections.abc import Iterable, Iterator, Mapping, Sequence
from copy import deepcopy
from os import PathLike, fspath
from pathlib import Path
from typing import TYPE_CHECKING, Any

from ._aci import describe_approximation, nearest_aci
from ._image import image_def_error
from ._viewport import layout_row
from ._write_preflight import (
    ATTRIBUTES,
    TextEncoder,
    WriteDiagnostic,
    row_strings,
    strict_losses,
)
from .drawing import (
    WRITE_BATCH,
    BlockTable,
    Drawing,
    EditableLayout,
    _COMMON,
    _table_rows,
)
from .entity import Entity

if TYPE_CHECKING:
    from .convert import WriteResult

#: Default ceiling of the entity handles; table and generated object handles
#: are numbered above it.
MAX_ENTITIES = 1 << 24

_TABLE_KIND = {"layers": "LAYER", "linetypes": "LTYPE", "text_styles": "STYLE"}


def new_streaming(
    version: str = "AC1015", *, max_entities: int = MAX_ENTITIES
) -> StreamingDrawing:
    """Create a write-only drawing (see :class:`StreamingDrawing`)."""
    return StreamingDrawing(version, max_entities=max_entities)


class _TableDict(dict):
    """A table of a streaming drawing; rows set after the writer started are
    forwarded to it (a known name replaces that row)."""

    __slots__ = ("_doc", "_kind")

    def __init__(self, doc: StreamingDrawing, kind: str, rows: Mapping[str, Any]) -> None:
        super().__init__(rows)
        self._doc = doc
        self._kind = kind

    def __setitem__(self, name: str, props: Any) -> None:
        super().__setitem__(name, props)
        self._doc._table_changed(self._kind, name, props)

    def __delitem__(self, name: str) -> None:
        if self._doc._writer is not None:
            raise ValueError("table rows cannot be removed once entities have been written")
        super().__delitem__(name)


class StreamingLayout(EditableLayout):
    """A layout that writes each entity as it is added and keeps none of them."""

    def __init__(
        self,
        doc: StreamingDrawing,
        name: str = "MODELSPACE",
        *,
        attribute_owner: int | None = None,
        paper: bool = False,
    ) -> None:
        super().__init__(doc, name, attribute_owner=attribute_owner, paper=paper)
        self._count = 0

    def _owner(self) -> str | int | None:
        """The ``entity_owners`` value of this layout's entities."""
        if self._attribute_owner is not None:
            return self._attribute_owner
        if self._paper:
            return "*PAPER_SPACE"
        return None

    def _store(self, entity: Entity) -> None:
        # The editable drawing resolves block references when it saves; a
        # streaming drawing has to know them now.
        block = None
        if entity.dxftype == "INSERT":
            block = entity.dxf.get("name")
        elif entity.dxftype == "DIMENSION":
            block = entity.dxf.get("geometry")
        if block is not None and (not isinstance(block, str) or block not in self.doc.blocks):
            raise ValueError(f"undefined block: {block}")
        self.doc._emit(entity, self._owner())
        self._count += 1
        if entity.dxftype == "INSERT":
            self.doc._inserts.add(entity.handle)

    def __len__(self) -> int:
        return self._count

    def query(
        self, types: str | Iterable[str] | None = None, *, include_styles: bool = True
    ) -> Iterator[Entity]:
        """Nothing: a streaming layout does not keep its entities."""
        return iter(())

    def get(self, handle: int) -> Entity:
        raise KeyError(handle)

    def _resolve(self, entity: Entity | int) -> Entity:
        if isinstance(entity, Entity):
            return entity
        if entity in self.doc._inserts:
            return Entity("INSERT", entity, {})
        raise ValueError("entity does not belong to this layout")

    def update(self, entity: Entity | int, **attributes: Any) -> Entity:
        raise TypeError("a streaming drawing cannot update entities it has written")

    def delete(self, entity: Entity | int) -> None:
        raise TypeError("a streaming drawing cannot delete entities it has written")

    def add_attrib(
        self,
        blockref: Entity | int,
        tag: str,
        text: str,
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        height: float = 1.0,
        **attributes: Any,
    ) -> Entity:
        parent = self._resolve(blockref)
        if parent.dxftype != "INSERT":
            raise ValueError("ATTRIB owner must be INSERT")
        layout = self.doc._attributes.setdefault(
            parent.handle, StreamingLayout(self.doc, attribute_owner=parent.handle)
        )
        return layout.add_entity(
            "ATTRIB",
            {"tag": tag, "text": text, "insert": insert, "height": height, **attributes},
        )

    def attribs(self, blockref: Entity | int) -> tuple[Entity, ...]:
        raise TypeError("a streaming drawing does not keep the attributes it has written")


class StreamingBlockLayout(StreamingLayout):
    """Block contents written as they are added."""

    def __init__(
        self, doc: StreamingDrawing, name: str, base_point: Sequence[float], description: str
    ) -> None:
        super().__init__(doc, name)
        self.base_point = tuple(base_point) + ((0.0,) if len(base_point) == 2 else ())
        self.description = description

    def _owner(self) -> str:
        return self.name


class StreamingBlockTable(BlockTable):
    """Block definitions of a streaming drawing: created, never deleted."""

    def new(
        self,
        name: str,
        *,
        base_point: Sequence[float] = (0.0, 0.0, 0.0),
        description: str = "",
    ) -> StreamingBlockLayout:
        if not isinstance(name, str) or not name or name in self:
            raise ValueError(f"empty or duplicate block name: {name}")
        if name.upper().startswith(("*MODEL_SPACE", "*PAPER_SPACE")):
            raise ValueError("reserved block name")
        block = StreamingBlockLayout(self.doc, name, base_point, description)
        self._blocks[name.upper()] = block
        self.doc._block_added(block)
        return block

    def delete(self, name: str) -> None:
        raise TypeError("a streaming drawing cannot delete block definitions")


class StreamingDrawing(Drawing):
    """A write-only :class:`~ezdwg.drawing.Drawing`; see the module documentation.

    ``max_entities`` bounds the entity handles (1 up to the bound); handles of
    tables and generated objects are numbered above it.
    """

    def __init__(self, version: str = "AC1015", *, max_entities: int = MAX_ENTITIES) -> None:
        super().__init__(version)
        if not isinstance(max_entities, int) or max_entities < 1:
            raise ValueError("max_entities must be a positive integer")
        self.max_entities = max_entities
        self._writer: Any = None
        self._sink: Any = None
        self._encoder: TextEncoder | None = None
        self._stream_diagnostics: list[WriteDiagnostic] = []
        self._batch: list[Entity] = []
        self._batch_attrs: dict[int, dict[str, Any]] = {}
        self._batch_owners: dict[int, str | int] = {}
        #: Owners of the ATTRIBs written so far (they wait for the end).
        self._attrib_owners: dict[int, int] = {}
        #: Handles of the INSERTs written so far (for ``add_attrib``).
        self._inserts: set[int] = set()
        #: Block names as the writer stores them (code page encoded).
        self._block_names: dict[str, str] = {}
        self._written = False
        self._layout = StreamingLayout(self)
        self._paperspace = StreamingLayout(self, "PAPERSPACE", paper=True)
        self.blocks = StreamingBlockTable(self)
        self.layers = _TableDict(self, "layers", self.layers)
        self.linetypes = _TableDict(self, "linetypes", self.linetypes)
        self.text_styles = _TableDict(self, "text_styles", self.text_styles)
        self.image_defs = _TableDict(self, "image_defs", self.image_defs)

    @property
    def diagnostics(self) -> tuple[WriteDiagnostic, ...]:
        """Losses recorded so far (the entities are not kept, so none is filtered)."""
        return tuple(self._diagnostics) + tuple(self._stream_diagnostics)

    # ---------- writer ----------
    def _begin(self) -> None:
        """Fix the tables and the code page; open the native writer."""
        if self._writer is not None:
            return
        if self._written:
            raise ValueError("the streaming drawing has already been saved")
        from . import raw
        from .convert import _WriteSink

        version = self.version
        diagnostics = self._stream_diagnostics
        metadata = deepcopy(self.header)
        layer_rows = _table_rows(self.layers, "LAYER", version, diagnostics)
        ltype_rows = _table_rows(self.linetypes, None, version, diagnostics)
        style_rows = _table_rows(self.text_styles, None, version, diagnostics)
        image_rows: dict[str, dict[str, Any]] = {}
        for name, props in self.image_defs.items():
            image_def_error(name, props)
            image_rows[name] = {"name": name, **deepcopy(props)}
        block_rows = {
            block.name: {
                "name": block.name,
                "base_point": tuple(block.base_point),
                "description": block.description,
            }
            for block in self.blocks.values()
        }
        layout = layout_row(self.layout)
        # Without ``header["codepage"]`` the code page is chosen from the table
        # names known so far (the entities are not known yet).
        strings = row_strings(
            layer_rows.values(),
            ltype_rows.values(),
            style_rows.values(),
            block_rows.values(),
            image_rows.values(),
            [layout],
        )
        encoder = TextEncoder(strings, metadata, diagnostics, version=version)
        for table, kind in (
            (layer_rows, "LAYER"),
            (ltype_rows, "LTYPE"),
            (style_rows, "STYLE"),
            (block_rows, "BLOCK"),
            (image_rows, "IMAGEDEF"),
            ({"layout": layout}, "LAYOUT"),
        ):
            encoder.rows(table.values(), kind)
        self._block_names = {name: row["name"] for name, row in block_rows.items()}
        writer = raw.DwgWriter(version, max_handle=self.max_entities)
        writer.set_tables(
            layers=list(layer_rows.values()),
            linetypes=list(ltype_rows.values()),
            text_styles=list(style_rows.values()),
            metadata=metadata,
            blocks=list(block_rows.values()),
            image_defs=list(image_rows.values()) if version == "AC1015" else [],
            layout=layout,
        )
        self._encoder = encoder
        self._writer = writer
        self._sink = _WriteSink(writer, version, diagnostics)

    def _table_changed(self, kind: str, name: str, props: Any) -> None:
        if self._writer is None:
            return  # collected when the writer starts
        assert self._encoder is not None
        version, diagnostics = self.version, self._stream_diagnostics
        if kind == "image_defs":
            image_def_error(name, props)
            rows = [{"name": name, **deepcopy(props)}]
            self._encoder.rows(rows, "IMAGEDEF")
            if version == "AC1015":
                self._writer.add_tables(image_defs=rows)
            return
        rows = list(
            _table_rows({name: props}, "LAYER" if kind == "layers" else None, version, diagnostics)
            .values()
        )
        self._encoder.rows(rows, _TABLE_KIND[kind])
        self._writer.add_tables(**{kind: rows})

    def _block_added(self, block: StreamingBlockLayout) -> None:
        if self._writer is None:
            return
        assert self._encoder is not None
        row = {
            "name": block.name,
            "base_point": tuple(block.base_point),
            "description": block.description,
        }
        self._encoder.rows([row], "BLOCK")
        self._block_names[block.name] = row["name"]
        self._writer.add_tables(blocks=[row])

    def _emit(self, entity: Entity, owner: str | int | None) -> None:
        self._begin()
        assert self._encoder is not None
        if entity.handle > self.max_entities:
            raise ValueError(
                f"a streaming drawing holds at most {self.max_entities} entities (max_entities)"
            )
        data = entity.dxf
        prepared = self._encoder.entity(Entity(entity.dxftype, entity.handle, dict(data)))
        # VIEWPORT ``width`` is the window size, not the text width factor.
        common_keys = _COMMON if entity.dxftype == "VIEWPORT" else _COMMON | ATTRIBUTES | {"style"}
        attrs = {k: v for k, v in data.items() if k in common_keys and v is not None}
        # AC1015 has no true colour: round it to the nearest ACI and report.
        true_color = attrs.get("true_color")
        if self.version == "AC1015":
            attrs.pop("true_color", None)
            if true_color is not None:
                index = nearest_aci(true_color)
                attrs["color_index"] = index
                self._stream_diagnostics.append(
                    WriteDiagnostic(
                        entity.handle,
                        entity.dxftype,
                        "true_color",
                        describe_approximation(true_color, index),
                        "approximated",
                    )
                )
        attrs.setdefault("color_index", 256)
        self._encoder.attributes({entity.handle: attrs})
        self._batch.append(prepared)
        self._batch_attrs[entity.handle] = attrs
        if entity.dxftype == "ATTRIB":
            if isinstance(owner, int):
                self._attrib_owners[entity.handle] = owner
        elif owner is not None:
            self._batch_owners[entity.handle] = (
                self._block_names.get(owner, owner) if isinstance(owner, str) else owner
            )
        if len(self._batch) >= WRITE_BATCH:
            self._flush()

    def _flush(self) -> None:
        if self._batch:
            self._sink.emit(self._batch, self._batch_attrs, self._batch_owners)
            self._batch, self._batch_attrs, self._batch_owners = [], {}, {}

    def save(
        self, output_path: str | PathLike[str] | None = None, *, strict: bool = True
    ) -> WriteResult:
        """Write everything added so far and finish the file.

        A strict save fails before anything is written when an entity was
        skipped or a loss diagnostic is not acceptable. The drawing cannot be
        used afterwards.
        """
        from .convert import WriteResult
        from .document import clear_decode_caches

        destination = output_path if output_path is not None else self.path
        if destination is None:
            raise ValueError("a streaming drawing requires an output path")
        self._begin()
        assert self._encoder is not None
        self._flush()
        sink = self._sink
        sink.finish(self._attrib_owners)
        total, written = sink.total, sink.written
        skipped = total - written
        strict_diagnostics = strict_losses(self._stream_diagnostics)
        if strict and (skipped or strict_diagnostics):
            summary = ", ".join(f"{t}:{n}" for t, n in sorted(sink.skipped_by_type.items()))
            losses = "; ".join(
                f"{d.dxftype}[{d.handle}].{d.attribute}: {d.message}"
                for d in strict_diagnostics[:8]
            )
            raise ValueError(
                f"failed to write {skipped} entities ({summary}); "
                f"{len(strict_diagnostics)} diagnostics: {losses}"
            )
        # The sheet and the header may have changed after the first entity; the
        # code page is the one the strings were encoded with.
        metadata = deepcopy(self.header)
        requested = metadata.get("codepage")
        if self.version != "AC1032" and requested not in (None, self._encoder.codepage):
            raise ValueError("the code page is fixed once entities have been written")
        metadata["codepage"] = self._encoder.codepage
        layout = layout_row(self.layout)
        self._encoder.rows([layout], "LAYOUT")
        self._writer.set_tables(metadata=metadata, layout=layout)
        out_path = Path(fspath(destination))
        out_path.parent.mkdir(parents=True, exist_ok=True)
        self._writer.write(str(out_path))
        self._written = True
        self._writer = None
        self._sink = None
        clear_decode_caches()
        self.path = str(out_path)
        return WriteResult(
            source_path="",
            output_path=str(out_path),
            target_version=self.version,
            total_entities=total,
            written_entities=written,
            skipped_entities=skipped,
            skipped_by_type=dict(sorted(sink.skipped_by_type.items())),
            diagnostics=tuple(self._stream_diagnostics),
        )

