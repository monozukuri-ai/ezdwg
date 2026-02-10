import ezdwg


result = ezdwg.to_dxf(
    "examples/data/line_2000.dwg",
    "/tmp/line_2000_out.dxf",
    types="LINE",
    dxf_version="R2010",
)
print(result)
