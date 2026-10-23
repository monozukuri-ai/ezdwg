# ezdwg-core

Pure Rust DWG reader and native AC1015 (AutoCAD R2000) writer, used by the
[ezdwg Python package](https://github.com/monozukuri-ai/ezdwg).
This crate has no Python or PyO3 dependency.

## Install

```sh
cargo add ezdwg-core
```

## Read a DWG file

The decoder supports R13 / AC1012, R14 / AC1014, R2000 / AC1015,
R2004 / AC1018, R2007 / AC1021, R2010 / AC1024, R2013 / AC1027 and
R2018 / AC1032. Entity coverage varies by version; this is not a complete
implementation of the DWG format.

```no_run
use ezdwg_core::core::config::ParseConfig;
use ezdwg_core::dwg::{decoder::Decoder, file_open::read_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read_file("drawing.dwg")?;
    let decoder = Decoder::new(&bytes, ParseConfig::default())?;
    decoder.ensure_supported()?;

    println!("DWG version: {}", decoder.version().as_str());
    let index = decoder.build_object_index()?;
    println!("Objects: {}", index.len());
    for object in &index.objects {
        let record = decoder.parse_object_record(object.offset)?;
        println!("Handle {:?}: {} bytes", object.handle, record.size);
    }
    Ok(())
}
```

This is a low-level API: `Decoder` exposes sections, object records, header
variables and class maps. The `entities`, `objects` and `bit` modules provide
version-specific decoding primitives. The Python `Document` / `Layout` query
and DXF conversion APIs are separate from this crate.

## Write an AC1015 drawing

Build a `WriterDocument` and serialize it to bytes. Only AC1015 output is
currently supported; the writer validates the supported entity data before
encoding it.

```rust
use ezdwg_core::core::config::ParseConfig;
use ezdwg_core::dwg::decoder::Decoder;
use ezdwg_core::writer::{
    r2000::write_document, LineEntity, WriterConfig, WriterDocument, WriterEntity,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut drawing = WriterDocument::default();
    drawing.modelspace.push(WriterEntity::Line(LineEntity {
        common: Default::default(),
        start: (0.0, 0.0, 0.0),
        end: (10.0, 5.0, 0.0),
    }));

    let bytes = write_document(&drawing, &WriterConfig::default())?;
    // Save with std::fs::write("created.dwg", &bytes)? when needed.
    let decoder = Decoder::new(&bytes, ParseConfig::default())?;
    assert_eq!(decoder.version().as_str(), "AC1015");
    assert!(!decoder.build_object_index()?.is_empty());
    Ok(())
}
```

The Rust API is evolving and follows the shared ezdwg release version.
See the [API reference](https://docs.rs/ezdwg-core) and the
[project documentation](https://monozukuri-ai.github.io/ezdwg/) for details.
The repository's full regression suite uses DWG fixtures outside this crate;
run it from a source checkout. The packaged README examples can be tested
independently with `cargo test --doc`.

## License

MIT. See the bundled `LICENSE` file or the
[repository license](https://github.com/monozukuri-ai/ezdwg/blob/main/LICENSE).
