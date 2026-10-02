use crate::bit::{BitReader, Endian};
use crate::core::result::Result;
use crate::entities::common::{
    checked_handle_count, parse_common_entity_handles, parse_common_entity_header,
    parse_common_entity_header_r14_with_handle, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013, read_handle_reference,
    CommonEntityHeader,
};

/// The view of model space a paper-space viewport shows (R2000+; R13/R14 keep
/// it in the extended entity data of the viewport, which is not read).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportView {
    /// View target point in WCS (DXF group 17).
    pub target: (f64, f64, f64),
    /// View direction, from the target towards the camera (DXF group 16).
    pub direction: (f64, f64, f64),
    /// View twist angle in radians (DXF group 51, which is in degrees).
    pub twist_angle: f64,
    /// Height of the view in model units (DXF group 45).
    pub view_height: f64,
    /// Perspective lens length (DXF group 42).
    pub lens_length: f64,
    /// Front and back clip plane Z (DXF groups 43 and 44).
    pub front_clip_z: f64,
    pub back_clip_z: f64,
    /// View center in display coordinates (DXF groups 12 and 22).
    pub view_center: (f64, f64),
    /// Status flags (DXF group 90): 0x1 perspective, 0x800 hide plot,
    /// 0x10000 non-rectangular clipping, 0x20000 viewport off.
    pub status_flags: u32,
    /// Render mode (DXF group 281): 0 = 2D optimized.
    pub render_mode: u8,
}

/// Paper-space viewport: a window in a layout that shows model space.
#[derive(Debug, Clone)]
pub struct ViewportEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    /// Center of the viewport in paper space (DXF group 10), with its width
    /// and height (DXF groups 40 and 41). `None` when the body cannot be read.
    pub center: Option<(f64, f64, f64)>,
    pub width: f64,
    pub height: f64,
    pub view: Option<ViewportView>,
    /// Layers that are frozen in this viewport (DXF group 331).
    pub frozen_layer_handles: Vec<u64>,
    /// Entity that clips the viewport (DXF group 340).
    pub clip_boundary_handle: Option<u64>,
}

/// Version dependent parts of the viewport body (ODA specification 20.4.38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewportLayout {
    /// R13/R14: center, width and height only.
    R14,
    /// R2000 and R2004: view data with the plot style sheet name inline. They
    /// differ only behind the fields that are read here (R2004 adds the shade
    /// plot mode at the end of the body).
    R2000,
    /// R2007+: major grid lines; the style sheet name moves to the string
    /// stream.
    R2007Plus,
}

pub fn decode_viewport(reader: &mut BitReader<'_>) -> Result<ViewportEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_viewport_with_header(reader, header, ViewportLayout::R2000)
}

pub fn decode_viewport_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let header = parse_common_entity_header_r14_with_handle(reader, object_handle)?;
    decode_viewport_with_header(reader, header, ViewportLayout::R14)
}

pub fn decode_viewport_r2007(reader: &mut BitReader<'_>) -> Result<ViewportEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_viewport_with_header(reader, header, ViewportLayout::R2007Plus)
}

pub fn decode_viewport_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_viewport_with_header(reader, header, ViewportLayout::R2007Plus)
}

pub fn decode_viewport_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_viewport_with_header(reader, header, ViewportLayout::R2007Plus)
}

struct ViewportBody {
    center: (f64, f64, f64),
    width: f64,
    height: f64,
    view: Option<ViewportView>,
    frozen_layer_count: usize,
}

fn read_2rd(reader: &mut BitReader<'_>) -> Result<(f64, f64)> {
    Ok((
        reader.read_rd(Endian::Little)?,
        reader.read_rd(Endian::Little)?,
    ))
}

fn decode_viewport_body(
    reader: &mut BitReader<'_>,
    layout: ViewportLayout,
) -> Result<ViewportBody> {
    let center = reader.read_3bd()?;
    let width = reader.read_bd()?;
    let height = reader.read_bd()?;
    if layout == ViewportLayout::R14 {
        return Ok(ViewportBody {
            center,
            width,
            height,
            view: None,
            frozen_layer_count: 0,
        });
    }

    let target = reader.read_3bd()?;
    let direction = reader.read_3bd()?;
    let twist_angle = reader.read_bd()?;
    let view_height = reader.read_bd()?;
    let lens_length = reader.read_bd()?;
    let front_clip_z = reader.read_bd()?;
    let back_clip_z = reader.read_bd()?;
    let _snap_angle = reader.read_bd()?;
    let view_center = read_2rd(reader)?;
    let _snap_base = read_2rd(reader)?;
    let _snap_spacing = read_2rd(reader)?;
    let _grid_spacing = read_2rd(reader)?;
    let _circle_zoom = reader.read_bs()?;
    if layout == ViewportLayout::R2007Plus {
        let _grid_major = reader.read_bs()?;
    }
    let frozen_layer_count = reader.read_bl()? as usize;
    let status_flags = reader.read_bl()?;
    if layout != ViewportLayout::R2007Plus {
        let _style_sheet = reader.read_tv()?;
    }
    let render_mode = reader.read_rc()?;
    // The rest of the body (UCS of the viewport, shade plot mode, lighting)
    // carries nothing the handle stream depends on.

    Ok(ViewportBody {
        center,
        width,
        height,
        view: Some(ViewportView {
            target,
            direction,
            twist_angle,
            view_height,
            lens_length,
            front_clip_z,
            back_clip_z,
            view_center,
            status_flags,
            render_mode,
        }),
        frozen_layer_count,
    })
}

fn decode_viewport_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    layout: ViewportLayout,
) -> Result<ViewportEntity> {
    // A body that cannot be read still leaves the layer and color of the
    // viewport, which come from the common data and the handle stream.
    let body = decode_viewport_body(reader, layout)
        .ok()
        .filter(is_plausible_viewport_body);

    reader.set_bit_pos(header.obj_size);
    let common_handles = parse_common_entity_handles(reader, &header)?;

    let mut frozen_layer_handles = Vec::new();
    let mut clip_boundary_handle = None;
    if let Some(body) = body.as_ref().filter(|_| layout != ViewportLayout::R14) {
        // Frozen layer handles and the clip boundary follow the common
        // handles; a count that does not fit drops them, not the entity.
        if let Ok(count) = checked_handle_count(reader, body.frozen_layer_count, "frozen layer") {
            let read = (|| -> Result<(Vec<u64>, u64)> {
                let mut layers = Vec::with_capacity(count);
                for _ in 0..count {
                    layers.push(read_handle_reference(reader, header.handle)?);
                }
                let clip = read_handle_reference(reader, header.handle)?;
                Ok((layers, clip))
            })();
            if let Ok((layers, clip)) = read {
                frozen_layer_handles = layers.into_iter().filter(|handle| *handle != 0).collect();
                clip_boundary_handle = (clip != 0).then_some(clip);
            }
        }
    }

    Ok(ViewportEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        center: body.as_ref().map(|body| body.center),
        width: body.as_ref().map_or(0.0, |body| body.width),
        height: body.as_ref().map_or(0.0, |body| body.height),
        view: body.as_ref().and_then(|body| body.view),
        frozen_layer_handles,
        clip_boundary_handle,
    })
}

fn is_plausible_viewport_body(body: &ViewportBody) -> bool {
    let finite3 = |p: (f64, f64, f64)| p.0.is_finite() && p.1.is_finite() && p.2.is_finite();
    if !finite3(body.center) || !body.width.is_finite() || !body.height.is_finite() {
        return false;
    }
    if body.width < 0.0 || body.height < 0.0 {
        return false;
    }
    match body.view {
        None => true,
        Some(view) => {
            finite3(view.target)
                && finite3(view.direction)
                && view.twist_angle.is_finite()
                && view.view_height.is_finite()
                && view.view_center.0.is_finite()
                && view.view_center.1.is_finite()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decode_viewport;
    use crate::bit::{BitReader, BitWriter, Endian};

    /// Minimal viewport object stream: common entity header, body and the
    /// handle stream at `obj_size` bits. `r2000` stores "Nolinks" where R2004
    /// stores "XDic Missing Flag", and always has the xdictionary handle.
    fn build_viewport_stream(r2000: bool, frozen_layers: &[u64]) -> Vec<u8> {
        let build = |obj_size: u32| -> (Vec<u8>, u32) {
            let mut w = BitWriter::new();
            w.write_rl(Endian::Little, obj_size).unwrap();
            w.write_h(4, 0x2A).unwrap(); // handle
            w.write_bs(0).unwrap(); // no EED
            w.write_b(0).unwrap(); // no proxy graphics
            w.write_bb(2).unwrap(); // entity mode: model space (no owner handle)
            w.write_bl(0).unwrap(); // reactors
            w.write_b(1).unwrap(); // R2004: xdic missing / R2000: no links
            w.write_b(1).unwrap(); // no links
            w.write_b(0).unwrap(); // color
            w.write_bd(1.0).unwrap(); // ltype scale
            w.write_bb(0).unwrap(); // ltype flags
            w.write_bb(0).unwrap(); // plotstyle flags
            w.write_bs(0).unwrap(); // invisibility
            w.write_rc(0).unwrap(); // lineweight

            w.write_3bd(5.25, 4.0, 0.0).unwrap(); // center
            w.write_bd(8.4).unwrap(); // width
            w.write_bd(6.4).unwrap(); // height
            w.write_3bd(0.0, 0.0, 0.0).unwrap(); // view target
            w.write_3bd(0.0, 0.0, 1.0).unwrap(); // view direction
            w.write_bd(0.0).unwrap(); // twist angle
            w.write_bd(9.25).unwrap(); // view height
            w.write_bd(50.0).unwrap(); // lens length
            w.write_bd(0.0).unwrap(); // front clip
            w.write_bd(0.0).unwrap(); // back clip
            w.write_bd(0.0).unwrap(); // snap angle
            for value in [6.0, 4.5, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5] {
                // view center, snap base, snap spacing, grid spacing
                w.write_rd(Endian::Little, value).unwrap();
            }
            w.write_bs(100).unwrap(); // circle zoom
            w.write_bl(frozen_layers.len() as u32).unwrap();
            w.write_bl(0x8260).unwrap(); // status flags
            w.write_tv("").unwrap(); // style sheet
            w.write_rc(0).unwrap(); // render mode

            let handles_at = w.tell_bits() as u32;
            if r2000 {
                w.write_h(4, 0).unwrap(); // xdictionary
            }
            w.write_h(5, 0x10).unwrap(); // layer
            for handle in frozen_layers {
                w.write_h(4, *handle).unwrap();
            }
            w.write_h(4, 0x77).unwrap(); // clip boundary
            (w.into_bytes(), handles_at)
        };
        let (_, handles_at) = build(0);
        build(handles_at).0
    }

    #[test]
    fn viewport_reads_view_and_frozen_layers() {
        let bytes = build_viewport_stream(false, &[0x31, 0x32]);
        let mut reader = BitReader::new(&bytes);
        let entity = decode_viewport(&mut reader).expect("decode viewport");
        assert_eq!(entity.handle, 0x2A);
        assert_eq!(entity.layer_handle, 0x10);
        assert_eq!(entity.center, Some((5.25, 4.0, 0.0)));
        assert_eq!((entity.width, entity.height), (8.4, 6.4));
        let view = entity.view.expect("view");
        assert_eq!(view.view_center, (6.0, 4.5));
        assert_eq!(view.view_height, 9.25);
        assert_eq!(view.direction, (0.0, 0.0, 1.0));
        assert_eq!(view.status_flags, 0x8260);
        assert_eq!(entity.frozen_layer_handles, vec![0x31, 0x32]);
        assert_eq!(entity.clip_boundary_handle, Some(0x77));
    }

    #[test]
    fn r2000_viewport_skips_the_xdictionary_handle() {
        let bytes = build_viewport_stream(true, &[0x31]);
        let mut reader = BitReader::new(&bytes);
        reader.set_pre_r2004_layout(true);
        let entity = decode_viewport(&mut reader).expect("decode R2000 viewport");
        assert_eq!(entity.layer_handle, 0x10);
        assert_eq!(entity.frozen_layer_handles, vec![0x31]);
        assert_eq!(entity.clip_boundary_handle, Some(0x77));
        assert_eq!(entity.view.expect("view").view_height, 9.25);
    }
}
