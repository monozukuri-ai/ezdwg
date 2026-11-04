"""Provide compact catalog views from unit tests' mocked object headers."""

from ezdwg import raw


def mock_object_catalog(monkeypatch, headers) -> None:
    def type_names(path):
        return list(dict.fromkeys(row[4] for row in headers(path)))

    def handles(path, type_name, limit=None):
        target = type_name.strip().upper()
        found = [row[0] for row in headers(path) if row[4].strip().upper() == target]
        return found if limit is None else found[:limit]

    monkeypatch.setattr(raw, "list_object_headers_with_type", headers)
    monkeypatch.setattr(raw, "list_object_type_names", type_names)
    monkeypatch.setattr(raw, "list_object_handles_by_type_name", handles)
