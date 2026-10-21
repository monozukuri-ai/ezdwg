use super::common::{
    encode_entity_payload, encode_entity_payload_with_handles, CommonEntityEncodeInput,
};
use super::text::{write_text_body, TextEncodeInput};
use crate::bit::Endian;
use crate::core::result::Result;
use crate::writer::ir::{AttributeEntity, InsertEntity};

pub fn encode_insert(
    common: CommonEntityEncodeInput,
    insert: &InsertEntity,
    block: u64,
    attributes: &[u64],
    seqend: u64,
) -> Result<Vec<u8>> {
    let mut refs = vec![(5, block)];
    if let (Some(first), Some(last)) = (attributes.first(), attributes.last()) {
        refs.extend([(4, *first), (4, *last), (3, seqend)]);
    }
    encode_entity_payload_with_handles(
        7,
        common,
        |w| {
            w.write_3bd(insert.insert.0, insert.insert.1, insert.insert.2)?;
            // Explicit per-axis scale preserves reflections and nonuniform scaling.
            w.write_bb(0)?;
            w.write_rd(Endian::Little, insert.scale.0)?;
            w.write_dd(insert.scale.0, insert.scale.1)?;
            w.write_dd(insert.scale.0, insert.scale.2)?;
            w.write_bd(insert.rotation_rad)?;
            let normal = common.formatting.extrusion;
            w.write_3bd(normal.0, normal.1, normal.2)?;
            w.write_b(u8::from(!attributes.is_empty()))
        },
        &refs,
    )
}

pub fn encode_attribute(
    common: CommonEntityEncodeInput,
    attr: &AttributeEntity,
    definition: bool,
) -> Result<Vec<u8>> {
    let text = TextEncodeInput {
        common,
        text: attr.text.text.clone(),
        insertion: attr.text.insert,
        height: attr.text.height,
        rotation: attr.text.rotation_rad,
    };
    encode_entity_payload(if definition { 3 } else { 2 }, common, |w| {
        write_text_body(w, &text)?;
        w.write_tv(&attr.tag)?;
        w.write_bs(attr.field_length)?;
        w.write_rc(attr.flags)?;
        if definition {
            w.write_tv(&attr.prompt)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

    fn common() -> CommonEntityEncodeInput {
        CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            entity_mode: 0,
            layer_handle: 20,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: Default::default(),
            previous_handle: 0,
            next_handle: 0,
            style_handle: 30,
        }
    }

    #[test]
    fn insert_retains_nonuniform_reflection_normal_and_attribute_handle_chain() {
        let input = InsertEntity {
            common: Default::default(),
            block_name: "Part".into(),
            insert: (1., 2., 3.),
            scale: (-2., 3., 4.),
            rotation_rad: 0.25,
            attributes: Vec::new(),
        };
        let mut c = common();
        c.formatting.extrusion = (0., 1., 0.);
        let bytes = encode_insert(c, &input, 50, &[200, 300], 400).unwrap();
        let mut r = BitReader::new(&bytes);
        r.set_pre_r2004_layout(true);
        assert_eq!(r.read_bs().unwrap(), 7);
        let mut geometry = r.clone();
        let decoded = crate::entities::insert::decode_insert(&mut geometry).unwrap();
        assert_eq!(decoded.scale, input.scale);
        assert_eq!(decoded.extrusion, c.formatting.extrusion);
        assert_eq!(decoded.block_header_handle, Some(50));
        let header = parse_common_entity_header(&mut r).unwrap();
        r.set_bit_pos(header.obj_size);
        assert_eq!(
            parse_common_entity_handles(&mut r, &header)
                .unwrap()
                .owner_ref,
            Some(10)
        );
        let refs: Vec<_> = (0..4)
            .map(|_| {
                let h = r.read_h().unwrap();
                (h.code, h.value)
            })
            .collect();
        assert_eq!(refs, [(5, 50), (4, 200), (4, 300), (3, 400)]);
    }

    #[test]
    fn attdef_and_attrib_reuse_text_geometry_but_keep_attribute_tail_and_style() {
        let attr = AttributeEntity {
            text: crate::writer::TextEntity {
                text: "Value".into(),
                insert: (1., 2., 3.),
                height: 2.,
                ..Default::default()
            },
            tag: "ID".into(),
            prompt: "Identifier?".into(),
            flags: 8,
            field_length: 0,
        };
        for definition in [true, false] {
            let bytes = encode_attribute(common(), &attr, definition).unwrap();
            let mut r = BitReader::new(&bytes);
            r.set_pre_r2004_layout(true);
            assert_eq!(r.read_bs().unwrap(), if definition { 3 } else { 2 });
            let decoded = if definition {
                crate::entities::attrib::decode_attdef(&mut r)
            } else {
                crate::entities::attrib::decode_attrib(&mut r)
            }
            .unwrap();
            assert_eq!(decoded.text, "Value");
            assert_eq!(decoded.tag.as_deref(), Some("ID"));
            assert_eq!(decoded.flags, 8);
            assert_eq!(decoded.style_handle, Some(30));
            assert_eq!(decoded.owner_handle, Some(10));
            if definition {
                assert_eq!(decoded.prompt.as_deref(), Some("Identifier?"));
            }
        }
    }
}
