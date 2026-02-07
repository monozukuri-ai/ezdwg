import ezdwg


def main() -> None:
    doc = ezdwg.read("examples/data/mechanical_example-imperial.dwg")
    msp = doc.modelspace()

    dims = list(msp.query("DIMENSION"))
    print(f"DIMENSION count: {len(dims)}")
    if dims:
        first = dims[0]
        print("first:", first.handle, first.dxf.get("text"), first.dxf.get("actual_measurement"))

    ax = msp.plot(types="DIMENSION", show=False, title="Dimensions")
    ax.figure.savefig("dimensions.png", dpi=150)
    print("saved: dimensions.png")


if __name__ == "__main__":
    main()
