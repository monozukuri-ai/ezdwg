from __future__ import annotations

from pathlib import Path

import ezdwg


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"
AC1032_SAMPLE = SAMPLES / "acadsharp" / "sample_AC1032.dwg"


def test_raw_decode_document_graph_contains_object_entity_and_tables() -> None:
    version, objects, entities, edges, layers, block_headers, header_handles = (
        ezdwg.raw.decode_document_graph(str(AC1032_SAMPLE))
    )

    assert version == "AC1032"
    assert objects
    assert entities
    assert edges
    assert layers

    object_type_names = {type_name for *_prefix, type_name, _type_class in objects}
    assert "INSERT" in object_type_names

    entity_type_names = {type_name for _handle, type_name, *_rest in entities}
    assert entity_type_names - {"UNKNOWN"}

    edge_kinds = {kind for _source, kind, _target in edges}
    assert "handle_ref" in edge_kinds

    block_names = {name for _handle, name in block_headers}
    assert "*Model_Space" in block_names

    header_handle_map = dict(header_handles)
    assert header_handle_map["model_space_block_header"] in {
        handle for handle, name in block_headers if name == "*Model_Space"
    }
    assert header_handle_map["paper_space_block_header"] in {
        handle for handle, name in block_headers if name.startswith("*Paper_Space")
    }
    assert header_handle_map["clayer"] in {handle for handle, *_rest in layers}
    assert header_handle_map["layer_control"] in {
        handle
        for handle, *_rest, type_name, _type_class in objects
        if type_name == "LAYER_CONTROL"
    }


def test_read_graph_returns_typed_ir() -> None:
    graph = ezdwg.read_graph(AC1032_SAMPLE)

    assert graph.version == "AC1032"
    assert graph.objects
    assert graph.entities
    assert graph.edges
    assert graph.layers

    assert graph.entities[0].owner_handle is None or isinstance(
        graph.entities[0].owner_handle, int
    )
    assert isinstance(graph.entities[0].reactor_handles, tuple)

    model_handle = graph.header_handles.model_space_block_header
    assert model_handle is not None
    model_block = graph.get_block_header(model_handle)
    assert model_block is not None
    assert model_block.name == "*Model_Space"

    first_object = graph.objects[0]
    assert graph.get_object(first_object.handle) == first_object
    assert graph.edges_from(graph.edges[0].source_handle)
    assert graph.edges_to(graph.edges[0].target_handle)


def test_document_graph_method_uses_decode_path() -> None:
    doc = ezdwg.Document(path=str(AC1032_SAMPLE), version="AC1032")

    graph = doc.graph()

    assert graph.version == "AC1032"
    assert graph.header_handles.model_space_block_header is not None
    assert graph.header_handles.layer_control is not None
    assert graph.header_handles.clayer is not None
    assert graph.header_handles.get("clayer") == graph.header_handles.clayer
