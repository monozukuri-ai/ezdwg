//! IMAGE (class-based `AcDbRasterImage`, ODA 20.4.80) and its support objects
//! IMAGEDEF, IMAGEDEF_REACTOR and RASTERVARIABLES (20.4.81, 20.4.82, 20.4.91).
use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;
use crate::writer::ir::{ImageDef, ImageEntity};

use super::common::{encode_entity_payload_with_handles, CommonEntityEncodeInput};

pub const IMAGE_CLASS_VERSION: u32 = 0;

pub fn encode_image_entity_payload(
    type_code: u16,
    common: CommonEntityEncodeInput,
    image: &ImageEntity,
    image_def_handle: u64,
    reactor_handle: u64,
) -> Result<Vec<u8>> {
    encode_entity_payload_with_handles(
        type_code,
        common,
        |w| {
            w.write_bl(IMAGE_CLASS_VERSION)?;
            w.write_3bd(image.insert.0, image.insert.1, image.insert.2)?;
            w.write_3bd(image.u_vector.0, image.u_vector.1, image.u_vector.2)?;
            w.write_3bd(image.v_vector.0, image.v_vector.1, image.v_vector.2)?;
            w.write_rd(Endian::Little, image.size.0)?;
            w.write_rd(Endian::Little, image.size.1)?;
            w.write_bs(image.display_props)?;
            w.write_b(u8::from(image.clipping))?;
            w.write_rc(image.brightness)?;
            w.write_rc(image.contrast)?;
            w.write_rc(image.fade)?;
            w.write_bs(image.clip_type)?;
            if image.clip_type != 1 {
                w.write_bl(image.clip_vertices.len() as u32)?;
            }
            for vertex in &image.clip_vertices {
                w.write_rd(Endian::Little, vertex.0)?;
                w.write_rd(Endian::Little, vertex.1)?;
            }
            Ok(())
        },
        &[(5, image_def_handle), (3, reactor_handle)],
    )
}

/// Common object record with explicit reactors (objects, not entities).
fn object_with_reactors<F>(
    codepage: u16,
    type_code: u16,
    handle: u64,
    owner: u64,
    reactors: &[u64],
    refs: &[(u8, u64)],
    body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    let mut w = BitWriter::with_codepage(codepage);
    w.write_bs(type_code)?;
    let size_pos = w.tell_bits() as u32;
    w.write_rl(Endian::Little, 0)?;
    w.write_h(0, handle)?;
    w.write_bs(0)?; // EED size
    w.write_bl(reactors.len() as u32)?;
    body(&mut w)?;
    let end = w.tell_bits() as u32;
    w.set_bit_pos(size_pos);
    w.write_rl(Endian::Little, end)?;
    w.set_bit_pos(end);
    w.write_h(4, owner)?;
    for reactor in reactors {
        w.write_h(4, *reactor)?;
    }
    w.write_h(3, 0)?; // xdictionary
    for &(code, h) in refs {
        w.write_h(code, h)?;
    }
    Ok(w.into_bytes())
}

pub fn encode_image_def_object(
    codepage: u16,
    type_code: u16,
    handle: u64,
    owner: u64,
    reactors: &[u64],
    def: &ImageDef,
) -> Result<Vec<u8>> {
    object_with_reactors(codepage, type_code, handle, owner, reactors, &[], |w| {
        w.write_bl(0)?; // class version
        w.write_rd(Endian::Little, def.size.0)?;
        w.write_rd(Endian::Little, def.size.1)?;
        w.write_tv(&def.file_path)?;
        w.write_b(u8::from(def.is_loaded))?;
        w.write_rc(def.resolution_units)?;
        w.write_rd(Endian::Little, def.pixel_size.0)?;
        w.write_rd(Endian::Little, def.pixel_size.1)
    })
}

pub fn encode_image_def_reactor_object(
    codepage: u16,
    type_code: u16,
    handle: u64,
    image_handle: u64,
) -> Result<Vec<u8>> {
    object_with_reactors(codepage, type_code, handle, image_handle, &[], &[], |w| {
        w.write_bl(2) // class version
    })
}

pub fn encode_raster_variables_object(
    codepage: u16,
    type_code: u16,
    handle: u64,
    owner: u64,
) -> Result<Vec<u8>> {
    object_with_reactors(codepage, type_code, handle, owner, &[], &[], |w| {
        w.write_bl(0)?; // class version
        w.write_bs(0)?; // display frame
        w.write_bs(1)?; // display quality: high
        w.write_bs(0) // units: none
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::entities::image::decode_image;
    use crate::writer::ir::{CommonEntityProps, EntityFormatting};

    #[test]
    fn image_entity_roundtrips_through_the_reader() {
        let common = CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            entity_mode: 0,
            layer_handle: 20,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: EntityFormatting::default(),
            previous_handle: 0,
            next_handle: 0,
            style_handle: 0,
        };
        let image = ImageEntity {
            common: CommonEntityProps::default(),
            image_def: "logo".into(),
            insert: (1., 2., 0.),
            u_vector: (0.5, 0., 0.),
            v_vector: (0., 0.5, 0.),
            size: (640., 480.),
            display_props: 7,
            clipping: true,
            brightness: 50,
            contrast: 60,
            fade: 10,
            clip_type: 1,
            clip_vertices: vec![(-0.5, -0.5), (639.5, 479.5)],
        };
        let payload = encode_image_entity_payload(503, common, &image, 0x40, 0x41).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 503);
        let decoded = decode_image(&mut reader).unwrap();
        assert_eq!(decoded.handle, 100);
        assert_eq!(decoded.layer_handle, 20);
        assert_eq!(decoded.insert, (1., 2., 0.));
        assert_eq!(decoded.u_vector, (0.5, 0., 0.));
        assert_eq!(decoded.v_vector, (0., 0.5, 0.));
        assert_eq!(decoded.size, (640., 480.));
        assert_eq!(decoded.display_props, 7);
        assert!(decoded.clipping);
        assert_eq!(
            (decoded.brightness, decoded.contrast, decoded.fade),
            (50, 60, 10)
        );
        assert_eq!(decoded.clip_type, 1);
        assert_eq!(decoded.clip_vertices, vec![(-0.5, -0.5), (639.5, 479.5)]);
        assert_eq!(decoded.image_def_handle, 0x40);
        assert_eq!(decoded.reactor_handle, 0x41);
    }
}
