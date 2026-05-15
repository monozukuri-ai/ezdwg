from __future__ import annotations

from dataclasses import dataclass
from os import PathLike

from . import raw


@dataclass(frozen=True, slots=True)
class ObjectNode:
    handle: int
    offset: int
    data_size: int
    type_code: int
    type_name: str
    type_class: str


@dataclass(frozen=True, slots=True)
class EntityCommonData:
    handle: int
    type_name: str
    owner_handle: int | None
    color_index: int | None
    true_color: int | None
    layer_handle: int
    linetype_handle: int | None
    material_handle: int | None
    plotstyle_handle: int | None
    extension_dict_handle: int | None
    reactor_handles: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class ObjectEdge:
    source_handle: int
    kind: str
    target_handle: int


@dataclass(frozen=True, slots=True)
class LayerRecord:
    handle: int
    name: str | None
    color_index: int | None
    true_color: int | None


@dataclass(frozen=True, slots=True)
class BlockHeaderRecord:
    handle: int
    name: str


@dataclass(frozen=True, slots=True)
class HeaderHandles:
    rows: tuple[tuple[str, int | None], ...] = ()
    model_space_block_header: int | None = None
    paper_space_block_header: int | None = None
    clayer: int | None = None
    textstyle: int | None = None
    celtype: int | None = None
    cmaterial: int | None = None
    dimstyle: int | None = None
    cmlstyle: int | None = None
    block_control: int | None = None
    layer_control: int | None = None
    style_control: int | None = None
    shapefile_control: int | None = None
    linetype_control: int | None = None
    view_control: int | None = None
    ucs_control: int | None = None
    vport_control: int | None = None
    appid_control: int | None = None
    dimstyle_control: int | None = None
    viewport_entity_header_control: int | None = None
    dictionary_named_objects: int | None = None
    dictionary_layouts: int | None = None
    dictionary_plotsettings: int | None = None
    dictionary_plotstyles: int | None = None
    dictionary_materials: int | None = None
    dictionary_colors: int | None = None
    dictionary_visualstyle: int | None = None
    bylayer: int | None = None
    byblock: int | None = None
    continuous: int | None = None

    def get(self, name: str) -> int | None:
        return next((handle for label, handle in self.rows if label == name), None)


@dataclass(frozen=True, slots=True)
class DocumentGraph:
    version: str
    objects: tuple[ObjectNode, ...]
    entities: tuple[EntityCommonData, ...]
    edges: tuple[ObjectEdge, ...]
    layers: tuple[LayerRecord, ...]
    block_headers: tuple[BlockHeaderRecord, ...]
    header_handles: HeaderHandles

    def get_object(self, handle: int) -> ObjectNode | None:
        return next((obj for obj in self.objects if obj.handle == handle), None)

    def get_layer(self, handle: int) -> LayerRecord | None:
        return next((layer for layer in self.layers if layer.handle == handle), None)

    def get_block_header(self, handle: int) -> BlockHeaderRecord | None:
        return next((block for block in self.block_headers if block.handle == handle), None)

    def edges_from(self, handle: int, kind: str | None = None) -> tuple[ObjectEdge, ...]:
        return tuple(
            edge
            for edge in self.edges
            if edge.source_handle == handle and (kind is None or edge.kind == kind)
        )

    def edges_to(self, handle: int, kind: str | None = None) -> tuple[ObjectEdge, ...]:
        return tuple(
            edge
            for edge in self.edges
            if edge.target_handle == handle and (kind is None or edge.kind == kind)
        )


def read_graph(path: str | PathLike[str], limit: int | None = None) -> DocumentGraph:
    (
        version,
        object_rows,
        entity_rows,
        edge_rows,
        layer_rows,
        block_header_rows,
        header_handle_rows,
    ) = raw.decode_document_graph(str(path), limit)

    header_handle_map = dict(header_handle_rows)
    return DocumentGraph(
        version=version,
        objects=tuple(ObjectNode(*row) for row in object_rows),
        entities=tuple(
            EntityCommonData(*row[:-1], tuple(row[-1])) for row in entity_rows
        ),
        edges=tuple(ObjectEdge(*row) for row in edge_rows),
        layers=tuple(LayerRecord(*row) for row in layer_rows),
        block_headers=tuple(BlockHeaderRecord(*row) for row in block_header_rows),
        header_handles=HeaderHandles(
            rows=tuple(header_handle_rows),
            model_space_block_header=header_handle_map.get("model_space_block_header"),
            paper_space_block_header=header_handle_map.get("paper_space_block_header"),
            clayer=header_handle_map.get("clayer"),
            textstyle=header_handle_map.get("textstyle"),
            celtype=header_handle_map.get("celtype"),
            cmaterial=header_handle_map.get("cmaterial"),
            dimstyle=header_handle_map.get("dimstyle"),
            cmlstyle=header_handle_map.get("cmlstyle"),
            block_control=header_handle_map.get("block_control"),
            layer_control=header_handle_map.get("layer_control"),
            style_control=header_handle_map.get("style_control"),
            shapefile_control=header_handle_map.get("shapefile_control"),
            linetype_control=header_handle_map.get("linetype_control"),
            view_control=header_handle_map.get("view_control"),
            ucs_control=header_handle_map.get("ucs_control"),
            vport_control=header_handle_map.get("vport_control"),
            appid_control=header_handle_map.get("appid_control"),
            dimstyle_control=header_handle_map.get("dimstyle_control"),
            viewport_entity_header_control=header_handle_map.get(
                "viewport_entity_header_control"
            ),
            dictionary_named_objects=header_handle_map.get("dictionary_named_objects"),
            dictionary_layouts=header_handle_map.get("dictionary_layouts"),
            dictionary_plotsettings=header_handle_map.get("dictionary_plotsettings"),
            dictionary_plotstyles=header_handle_map.get("dictionary_plotstyles"),
            dictionary_materials=header_handle_map.get("dictionary_materials"),
            dictionary_colors=header_handle_map.get("dictionary_colors"),
            dictionary_visualstyle=header_handle_map.get("dictionary_visualstyle"),
            bylayer=header_handle_map.get("bylayer"),
            byblock=header_handle_map.get("byblock"),
            continuous=header_handle_map.get("continuous"),
        ),
    )
