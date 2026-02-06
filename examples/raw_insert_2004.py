from ezdwg import raw

path = "examples/data/insert_2004.dwg"

print("version:", raw.detect_version(path))
print("insert headers:", len(raw.list_object_headers_by_type(path, [0x07])))

for handle, x, y, z, sx, sy, sz, rotation in raw.decode_insert_entities(path):
    print(
        "INSERT",
        f"handle={handle}",
        f"pos=({x:.3f}, {y:.3f}, {z:.3f})",
        f"scale=({sx:.3f}, {sy:.3f}, {sz:.3f})",
        f"rotation={rotation:.6f}rad",
    )
