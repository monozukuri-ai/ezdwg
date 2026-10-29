//! Create a native DWG containing RGB colors and mixed-language text.
use ezdwg_core::dwg::version::DwgVersion;
use ezdwg_core::writer::{
    r2018, CommonEntityProps, LayerDef, LineEntity, TextEntity, WriterConfig, WriterDocument,
    WriterEntity,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layer = "図面_도면_Чертёж";
    let mut doc = WriterDocument {
        version: DwgVersion::R2018,
        ..Default::default()
    };
    doc.layers.push(LayerDef {
        name: layer.into(),
        true_color: Some(0x73b9ff),
        ..Default::default()
    });
    doc.modelspace.push(WriterEntity::Line(LineEntity {
        common: CommonEntityProps {
            layer_name: layer.into(),
            true_color: Some(0x123456),
            ..Default::default()
        },
        start: (0., 0., 0.),
        end: (10., 20., 0.),
    }));
    doc.modelspace.push(WriterEntity::Text(TextEntity {
        text: "日本語 한국어 Русский 中文 العربية 😀".into(),
        insert: (1., 2., 0.),
        height: 2.,
        ..Default::default()
    }));
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "unicode.dwg".into());
    std::fs::write(path, r2018::write_document(&doc, &WriterConfig::default())?)?;
    Ok(())
}
