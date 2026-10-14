"""Build an independent editable snapshot, retaining block ownership and losses."""

from __future__ import annotations

from copy import deepcopy
from os import fspath

from . import raw
from ._write_preflight import (
    WriteDiagnostic,
    geometry_error,
    prepare_attributes,
    strict_losses,
)
from ._dimension import kind_of
from .entity import Entity


def import_drawing(doc, *, strict=True, layout=None, types=None, all_blocks=True):
    from .convert import _resolve_dwg_export_entities
    from .drawing import _COMMON, _FIELDS, Drawing, EditableLayout, _normalize

    drawing = Drawing()
    drawing.source_path = fspath(doc.path)
    source = _resolve_dwg_export_entities(layout or doc.modelspace(), types)
    all_entities = {
        e.handle: e for e in _resolve_dwg_export_entities(doc.entities(), None)
    }
    all_entities.update((e.handle, e) for e in source)
    selected = {e.handle: e for e in source if e.dxftype != "ATTRIB"}
    diagnostics = []
    path = doc.decode_path or doc.path
    names = dict(raw.decode_block_header_names(path))
    metadata = {r["handle"]: r for r in raw.decode_write_blocks(path)}
    insert_metadata = {
        h: (normal, block, attr_handles)
        for h, normal, block, attr_handles in raw.decode_insert_write_properties(path)
    }
    named = {
        h: name
        for h, name in names.items()
        if not name.upper().startswith(("*MODEL_SPACE", "*PAPER_SPACE"))
    }
    owners = {}
    for e in all_entities.values():
        owner = e.dxf.get("owner_handle")
        if owner is None:
            placement = doc.entity_placement(e.handle)
            owner = placement[1] if placement else None
        owners[e.handle] = owner
        if e.dxftype == "INSERT":
            if e.handle in insert_metadata:
                normal, block, _ = insert_metadata[e.handle]
                e.dxf["extrusion"] = normal
                if block in names:
                    e.dxf["name"] = names[block]
            else:
                e.dxf["_insert_undecoded"] = True

    for e in all_entities.values():
        if e.dxftype == "DIMENSION":
            block = e.dxf.get("anonymous_block_name")
            if isinstance(block, str) and block and "geometry" not in e.dxf:
                e.dxf["geometry"] = block
            if "dimtype" in e.dxf:
                try:
                    e.dxf["dimtype"] = kind_of(e.dxf["dimtype"])
                except ValueError:
                    pass
    pending = list(named.values()) if all_blocks else []
    pending += [e.dxf.get("name") for e in selected.values() if e.dxftype == "INSERT"]
    pending += [
        e.dxf.get("geometry") for e in selected.values() if e.dxftype == "DIMENSION"
    ]
    imported = {}
    while pending:
        name = pending.pop()
        if not isinstance(name, str) or name.upper() in imported:
            continue
        handle = next((h for h, n in named.items() if n.upper() == name.upper()), None)
        if handle is None:
            continue  # Undefined INSERT is rejected before any output replacement.
        row = metadata.get(handle)
        block = drawing.blocks.new(
            name,
            base_point=row["base_point"] if row else (0, 0, 0),
            description=row["description"] if row else "",
        )
        imported[name.upper()] = handle
        if row is None:
            diagnostics.append(
                WriteDiagnostic(
                    handle,
                    "BLOCK",
                    "metadata",
                    f"block {name!r}: metadata could not be decoded",
                )
            )
        else:
            for loss in row["losses"]:
                diagnostics.append(
                    WriteDiagnostic(
                        handle,
                        "BLOCK",
                        loss,
                        f"block {name!r}: {loss} cannot be preserved",
                        "dropped",
                    )
                )
        for e in all_entities.values():
            if owners[e.handle] == handle and e.dxftype != "ATTRIB":
                selected[e.handle] = e
                if e.dxftype == "INSERT":
                    pending.append(e.dxf.get("name"))

    for e in all_entities.values():
        if (
            e.dxftype == "ATTRIB"
            and owners[e.handle] in selected
            and selected[owners[e.handle]].dxftype == "INSERT"
        ):
            selected[e.handle] = e
    links = (
        dict(raw.decode_attribute_links(path))
        if doc.version in {"AC1012", "AC1014", "AC1015"}
        else None
    )
    attribute_order = {}
    for parent in tuple(selected.values()):
        if parent.dxftype != "INSERT":
            continue
        refs = insert_metadata.get(parent.handle, (None, None, None))[2]
        ordered = []
        valid = refs is not None
        if refs:
            if links is None:
                ordered = refs
            else:
                current, last = refs[0], refs[-1]
                seen = set()
                while current and current not in seen:
                    seen.add(current)
                    ordered.append(current)
                    if current == last:
                        break
                    current = links.get(current, 0)
                valid = bool(ordered) and ordered[-1] == last
        valid = valid and len(ordered) == len(set(ordered))
        for h in ordered:
            child = all_entities.get(h)
            if (
                child is None
                or child.dxftype != "ATTRIB"
                or owners.get(h) not in (None, parent.handle)
            ):
                valid = False
                continue
            owners[h] = parent.handle
            selected[h] = child
        owned = {
            h
            for h, e in selected.items()
            if e.dxftype == "ATTRIB" and owners.get(h) == parent.handle
        }
        if not valid or owned != set(ordered):
            diagnostics.append(
                WriteDiagnostic(
                    parent.handle,
                    "INSERT",
                    "attributes",
                    "missing, inconsistent or undecoded owned attribute references",
                    "dropped",
                )
            )
        attribute_order[parent.handle] = ordered
    # An orphan or explicitly selected attribute is visible even without INSERT.
    selected.update(
        (e.handle, e)
        for e in source
        if e.dxftype == "ATTRIB" and e.handle not in selected
    )
    prepared, attrs, layers, ltypes, styles, header, losses, image_defs = (
        prepare_attributes(doc, list(selected.values()))
    )
    for row in image_defs:
        drawing.image_defs[row["name"]] = deepcopy(
            {k: v for k, v in row.items() if k != "name"}
        )
    diagnostics.extend(losses)
    for target, rows in (
        (drawing.layers, layers),
        (drawing.linetypes, ltypes),
        (drawing.text_styles, styles),
    ):
        for row in rows:
            for name in tuple(target):
                if name.upper() == row["name"].upper():
                    del target[name]
            target[row["name"]] = deepcopy(
                {k: v for k, v in row.items() if k != "name"}
            )
    drawing.header = {k: v for k, v in header.items() if k != "codepage"}
    block_by_handle = {h: drawing.blocks[name] for name, h in imported.items()}
    for e in prepared:
        allowed = _FIELDS.get(e.dxftype, set()) | _COMMON
        data = {k: v for k, v in e.dxf.items() if k in allowed}
        data.update({k: v for k, v in attrs[e.handle].items() if k in allowed})
        if e.dxftype == "MTEXT":
            data["text"] = e.dxf.get("raw_text", e.dxf.get("text", ""))
        if e.dxftype == "LWPOLYLINE" and "points" in data:
            data["points"] = [tuple(p[:2]) for p in data["points"]]
        if e.dxftype == "INSERT" and e.dxf.get("_insert_undecoded"):
            diagnostics.append(
                WriteDiagnostic(
                    e.handle,
                    "INSERT",
                    "references",
                    "INSERT normal/block reference could not be decoded",
                )
            )
        if e.dxftype in {"ATTDEF", "ATTRIB"} and doc.version == "AC1032":
            diagnostics.append(
                WriteDiagnostic(
                    e.handle,
                    e.dxftype,
                    "attribute_version",
                    "R2018 embedded multiline attribute properties are not qualified",
                    "dropped",
                )
            )
        copy = Entity(e.dxftype, e.handle, _normalize(data))
        owner = owners.get(e.handle)
        if (
            e.dxftype == "ATTRIB"
            and owner in selected
            and selected[owner].dxftype == "INSERT"
        ):
            target = drawing._attributes.setdefault(
                owner, EditableLayout(drawing, attribute_owner=owner)
            )
        else:
            target = block_by_handle.get(owner, drawing.modelspace())
        target._entities[e.handle] = copy
        error = geometry_error(copy)
        if copy.dxftype not in _FIELDS or error:
            diagnostics.append(
                WriteDiagnostic(
                    copy.handle,
                    copy.dxftype,
                    "geometry",
                    error or "unsupported or undecoded entity",
                    "skipped",
                )
            )
    for owner, order in attribute_order.items():
        target = drawing._attributes.get(owner)
        if target is not None:
            current = target._entities
            target._entities = {h: current[h] for h in order if h in current}
            target._entities.update(
                (h, e) for h, e in current.items() if h not in target._entities
            )
    if all_blocks:
        outside = [e for e in all_entities.values() if e.handle not in selected]
        if outside:
            diagnostics.append(
                WriteDiagnostic(
                    None,
                    "DOCUMENT",
                    "spaces",
                    f"{len(outside)} entities are outside the editable model-space/block subset (including paper space)",
                    "dropped",
                )
            )
    drawing._next_handle = max(selected, default=0) + 1
    drawing._diagnostics = tuple(diagnostics)
    losses = strict_losses(diagnostics)
    if strict and losses:
        detail = "; ".join(
            f"{d.dxftype}.{d.attribute}: {d.message}" for d in losses[:8]
        )
        raise ValueError(f"cannot create lossless editable snapshot: {detail}")
    return drawing
