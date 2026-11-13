//! MULTILEADER: a class-based leader whose label is a text or a block.
//!
//! The layout rests on a comparison, not on a description of the record.
//! Fields were placed by matching the bits of records with the values that
//! an independent DWG to DXF converter, used as a program only, writes for
//! the same objects, and by the order of the public DXF reference.
//! `decode_mleader*` returns a record only when its walk ends exactly at the
//! end of the data, of the string stream and of the handle stream.
//!
//! Limits, which the decoder keeps:
//! - More than one leader, line, point or block attribute is read as a plain
//!   repeat of the one the compared objects have. For attributes nothing
//!   checks that the handle, the numbers and the text of one attribute, which
//!   come from three streams, belong together.
//! - A block label was compared for R2013 and R2018 only. For the older
//!   layouts it is read on records that the tests build.
//! - A break in a leader or in a leader line, a text column height and,
//!   before R2010, an entry in the two lists of the tail have no known
//!   layout: a record that states one is refused.
//! - Fields with one value in all the evidence are walked and not returned
//!   (see the notes in the walk).
//! - R13 and R14 are not read.
use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013, read_handle_reference,
    string_stream_reader, CommonEntityHeader,
};

type Point3 = (f64, f64, f64);

/// A color as the record stores it: the color index, and from R2004 the
/// 32-bit value that a DXF file shows for the color (0 in R2000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MLeaderColor {
    pub index: u16,
    pub rgb: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderLine {
    pub index: u32,
    /// The points of the line, from the arrowhead on. The last point of the
    /// leader (`MLeaderRoot::last_point`) is not among them.
    pub points: Vec<Point3>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderRoot {
    pub has_last_point: bool,
    pub has_dogleg: bool,
    /// Where the leader lines meet, at the start of the dogleg.
    pub last_point: Point3,
    pub dogleg_vector: Point3,
    pub index: u32,
    pub dogleg_length: f64,
    pub lines: Vec<MLeaderLine>,
    /// R2010+: 0 horizontal, 1 vertical.
    pub attachment_direction: Option<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderText {
    /// The text with its MTEXT format codes.
    pub text: String,
    pub normal: Point3,
    pub style_handle: u64,
    pub location: Point3,
    pub direction: Point3,
    /// In radians, as in the DXF MULTILEADER.
    pub rotation: f64,
    pub width: f64,
    pub color: MLeaderColor,
    pub attachment: u16,
    /// The drawing direction of the text (DXF group 172).
    pub drawing_direction: u16,
    pub background_color: MLeaderColor,
    pub background_scale: f64,
    pub background_on: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderBlock {
    pub block_handle: u64,
    pub normal: Point3,
    pub location: Point3,
    pub scale: Point3,
    /// In radians, as in the DXF MULTILEADER.
    pub rotation: f64,
    pub color: MLeaderColor,
    /// The 16 values of the transformation matrix, in stored order (the
    /// order of the DXF group 47 values).
    pub matrix: [f64; 16],
}

/// A block attribute of a block label (R2010+).
#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderAttribute {
    /// The attribute definition (ATTDEF) of the block.
    pub definition_handle: u64,
    pub index: u16,
    pub width: f64,
    pub text: String,
}

/// The context data: what is drawn, in drawing units.
#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderContext {
    pub leaders: Vec<MLeaderRoot>,
    pub scale: f64,
    pub content_base: Point3,
    pub text_height: f64,
    pub arrow_size: f64,
    pub landing_gap: f64,
    pub left_attachment: u16,
    pub right_attachment: u16,
    pub text_alignment: u16,
    pub attachment_type: u16,
    pub text: Option<MLeaderText>,
    pub block: Option<MLeaderBlock>,
    pub plane_origin: Point3,
    pub plane_x_axis: Point3,
    pub plane_y_axis: Point3,
    pub plane_normal_reversed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MLeaderEntity {
    pub handle: u64,
    pub owner_handle: Option<u64>,
    pub layer_handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    /// R2010+ only.
    pub class_version: Option<u16>,
    pub context: MLeaderContext,
    pub style_handle: u64,
    pub override_flags: u32,
    pub leader_line_type: u16,
    pub leader_color: MLeaderColor,
    pub leader_linetype_handle: u64,
    /// As a DXF file shows it: -1 BYLAYER, -2 BYBLOCK, -3 the default.
    pub leader_lineweight: i32,
    pub landing_enabled: bool,
    pub dogleg_enabled: bool,
    pub dogleg_length: f64,
    /// The block of the arrowhead; 0 is the default arrowhead.
    pub arrow_handle: u64,
    pub arrow_size: f64,
    /// 0 none, 1 block, 2 text, 3 tolerance.
    pub content_type: u16,
    pub text_style_handle: u64,
    pub left_attachment: u16,
    pub right_attachment: u16,
    pub text_frame: bool,
    pub block_handle: u64,
    pub block_connection: u16,
    pub attributes: Vec<MLeaderAttribute>,
    /// Whether every number that was read is finite (also the numbers that
    /// are not returned).
    pub(crate) finite: bool,
}

impl MLeaderEntity {
    /// Whether every number of the record is finite.
    pub fn has_finite_numbers(&self) -> bool {
        self.finite
    }
}

/// The record layouts that differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Layout {
    /// Strings in the data, colors as an index.
    R2000,
    /// Strings in the data, colors with their 32-bit value.
    R2004,
    /// Strings in the string stream.
    R2007,
    /// A class version, per-line properties, block attributes, more
    /// attachment values.
    R2010,
    /// As R2010 with one more flag at the end (also R2018).
    R2013,
}

/// R2000 and R2004.
pub fn decode_mleader(reader: &mut BitReader<'_>) -> Result<MLeaderEntity> {
    let layout = if reader.pre_r2004_layout() {
        Layout::R2000
    } else {
        Layout::R2004
    };
    let header = parse_common_entity_header(reader)?;
    decode_mleader_with_header(reader, header, layout)
}

pub fn decode_mleader_r2007(reader: &mut BitReader<'_>) -> Result<MLeaderEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_mleader_with_header(reader, header, Layout::R2007)
}

pub fn decode_mleader_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MLeaderEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mleader_with_header(reader, header, Layout::R2010)
}

/// R2013 and R2018.
pub fn decode_mleader_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MLeaderEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mleader_with_header(reader, header, Layout::R2013)
}

fn format_error(message: impl Into<String>) -> DwgError {
    DwgError::new(ErrorKind::Format, message)
}

/// The least bits one item of a counted list takes in the data.
const LEADER_BITS: u64 = 22;
const LINE_BITS: u64 = 6;
const POINT_BITS: u64 = 6;
const ATTRIBUTE_BITS: u64 = 4;

/// The three streams of a record and what the walk learns on its way.
struct Walk<'a> {
    data: BitReader<'a>,
    data_end: u64,
    /// The string stream and the bit where it ends (R2007+).
    strings: Option<(BitReader<'a>, u64)>,
    handles: BitReader<'a>,
    base_handle: u64,
    layout: Layout,
    finite: bool,
}

impl Walk<'_> {
    fn flag(&mut self) -> Result<bool> {
        Ok(self.data.read_b()? != 0)
    }

    fn short(&mut self) -> Result<u16> {
        self.data.read_bs()
    }

    fn long(&mut self) -> Result<u32> {
        self.data.read_bl()
    }

    fn double(&mut self) -> Result<f64> {
        let value = self.data.read_bd()?;
        self.finite &= value.is_finite();
        Ok(value)
    }

    fn point(&mut self) -> Result<Point3> {
        Ok((self.double()?, self.double()?, self.double()?))
    }

    fn handle(&mut self) -> Result<u64> {
        read_handle_reference(&mut self.handles, self.base_handle)
    }

    fn text(&mut self) -> Result<String> {
        if self.layout < Layout::R2007 {
            return self.data.read_tv();
        }
        match self.strings.as_mut() {
            Some((strings, _)) => strings.read_tu(),
            None => Err(format_error(
                "MULTILEADER has a text field and no string stream",
            )),
        }
    }

    fn color(&mut self) -> Result<MLeaderColor> {
        let index = self.short()?;
        if self.layout == Layout::R2000 {
            return Ok(MLeaderColor { index, rgb: 0 });
        }
        let rgb = self.long()?;
        let names = self.data.read_rc()?;
        // A color name and a color book name, as for every color of this
        // kind. No object of the evidence has one.
        for bit in [0x01, 0x02] {
            if names & bit != 0 {
                self.text()?;
            }
        }
        Ok(MLeaderColor { index, rgb })
    }

    /// A count of items that take at least `item_bits` each in the data. It
    /// is checked against the bits the data has left before any memory is
    /// reserved for the items.
    fn count(&mut self, item_bits: u64, what: &str) -> Result<usize> {
        let count = u64::from(self.long()?);
        let left = self.data_end.saturating_sub(self.data.tell_bits());
        if count.saturating_mul(item_bits) > left {
            return Err(format_error(format!(
                "MULTILEADER {what} count {count} is larger than the record"
            )));
        }
        Ok(count as usize)
    }

    /// A count of items whose layout is not known: it must be 0.
    fn no_items(&mut self, what: &str) -> Result<()> {
        match self.long()? {
            0 => Ok(()),
            count => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("MULTILEADER has a {what} ({count}): the layout of that item is not known"),
            )),
        }
    }

    fn leaders(&mut self) -> Result<Vec<MLeaderRoot>> {
        let per_line_properties = self.layout >= Layout::R2010;
        let count = self.count(LEADER_BITS, "leader")?;
        let mut leaders = Vec::with_capacity(count);
        for _ in 0..count {
            let has_last_point = self.flag()?;
            let has_dogleg = self.flag()?;
            let last_point = self.point()?;
            let dogleg_vector = self.point()?;
            self.no_items("break in a leader")?;
            let index = self.long()?;
            let dogleg_length = self.double()?;
            let line_count = self.count(LINE_BITS, "leader line")?;
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                let point_count = self.count(POINT_BITS, "leader line point")?;
                let mut points = Vec::with_capacity(point_count);
                for _ in 0..point_count {
                    points.push(self.point()?);
                }
                self.no_items("break in a leader line")?;
                let index = self.long()?;
                if per_line_properties {
                    // Line type, color, linetype, lineweight, arrow size,
                    // arrowhead and flags of this one line. One value each
                    // in all the evidence (the null handles, 0 size and 0
                    // flags of a line that follows the object), so they are
                    // walked and not returned.
                    self.short()?;
                    self.color()?;
                    self.handle()?;
                    self.long()?;
                    self.double()?;
                    self.handle()?;
                    self.long()?;
                }
                lines.push(MLeaderLine { index, points });
            }
            let attachment_direction = if per_line_properties {
                Some(self.short()?)
            } else {
                None
            };
            leaders.push(MLeaderRoot {
                has_last_point,
                has_dogleg,
                last_point,
                dogleg_vector,
                index,
                dogleg_length,
                lines,
                attachment_direction,
            });
        }
        Ok(leaders)
    }

    fn text_content(&mut self) -> Result<MLeaderText> {
        let text = self.text()?;
        let normal = self.point()?;
        let style_handle = self.handle()?;
        let location = self.point()?;
        let direction = self.point()?;
        let rotation = self.double()?;
        let width = self.double()?;
        // A height (0 in all the evidence), the line spacing factor and the
        // line spacing style: one value each in all the evidence.
        self.double()?;
        self.double()?;
        self.short()?;
        let color = self.color()?;
        let attachment = self.short()?;
        let drawing_direction = self.short()?;
        let background_color = self.color()?;
        let background_scale = self.double()?;
        // Background transparency.
        self.long()?;
        let background_on = self.flag()?;
        // Background mask fill, column type, auto height, column width,
        // gutter, column order reversed: one value each in all the evidence.
        self.flag()?;
        self.short()?;
        self.flag()?;
        self.double()?;
        self.double()?;
        self.flag()?;
        self.no_items("text column height")?;
        // Word break.
        self.flag()?;
        Ok(MLeaderText {
            text,
            normal,
            style_handle,
            location,
            direction,
            rotation,
            width,
            color,
            attachment,
            drawing_direction,
            background_color,
            background_scale,
            background_on,
        })
    }

    fn block_content(&mut self) -> Result<MLeaderBlock> {
        let block_handle = self.handle()?;
        let normal = self.point()?;
        let location = self.point()?;
        let scale = self.point()?;
        let rotation = self.double()?;
        let color = self.color()?;
        let mut matrix = [0.0; 16];
        for value in &mut matrix {
            *value = self.double()?;
        }
        Ok(MLeaderBlock {
            block_handle,
            normal,
            location,
            scale,
            rotation,
            color,
            matrix,
        })
    }

    fn context(&mut self) -> Result<MLeaderContext> {
        let leaders = self.leaders()?;
        let scale = self.double()?;
        let content_base = self.point()?;
        let text_height = self.double()?;
        let arrow_size = self.double()?;
        let landing_gap = self.double()?;
        let left_attachment = self.short()?;
        let right_attachment = self.short()?;
        let text_alignment = self.short()?;
        let attachment_type = self.short()?;
        let text = if self.flag()? {
            Some(self.text_content()?)
        } else {
            None
        };
        let block = if self.flag()? {
            Some(self.block_content()?)
        } else {
            None
        };
        let plane_origin = self.point()?;
        let plane_x_axis = self.point()?;
        let plane_y_axis = self.point()?;
        let plane_normal_reversed = self.flag()?;
        if self.layout >= Layout::R2010 {
            // The top and the bottom attachment. Both are 9 in all the
            // evidence, so which one is first is not known.
            self.short()?;
            self.short()?;
        }
        Ok(MLeaderContext {
            leaders,
            scale,
            content_base,
            text_height,
            arrow_size,
            landing_gap,
            left_attachment,
            right_attachment,
            text_alignment,
            attachment_type,
            text,
            block,
            plane_origin,
            plane_x_axis,
            plane_y_axis,
            plane_normal_reversed,
        })
    }

    fn attributes(&mut self) -> Result<Vec<MLeaderAttribute>> {
        if self.layout < Layout::R2010 {
            // Two lists; both are empty in all the evidence.
            self.no_items("first list in its tail")?;
            self.no_items("second list in its tail")?;
            return Ok(Vec::new());
        }
        let count = self.count(ATTRIBUTE_BITS, "block attribute")?;
        let mut attributes = Vec::with_capacity(count);
        for _ in 0..count {
            attributes.push(MLeaderAttribute {
                definition_handle: self.handle()?,
                index: self.short()?,
                width: self.double()?,
                text: self.text()?,
            });
        }
        Ok(attributes)
    }

    /// Whether the walk used up the data, the string stream and the handle
    /// stream (which is padded to a whole byte), and did not run past them.
    /// The bit reader gives zero bits for the last 7 bits it does not have,
    /// so a position past the end is a handle the record does not hold.
    fn ends_exact(&self) -> bool {
        let strings_used_up = match &self.strings {
            Some((strings, end)) => strings.tell_bits() == *end,
            None => true,
        };
        let handles_used_up = match self
            .handles
            .total_bits()
            .checked_sub(self.handles.tell_bits())
        {
            Some(bits_left) => bits_left < 8,
            None => false,
        };
        self.data.tell_bits() == self.data_end && strings_used_up && handles_used_up
    }
}

fn decode_mleader_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    layout: Layout,
) -> Result<MLeaderEntity> {
    let mut handles = reader.clone();
    handles.set_bit_pos(header.obj_size);
    let common_handles = parse_common_entity_handles(&mut handles, &header)?;

    // The data ends where the handle stream begins. From R2007 the string
    // stream sits in between, in front of a flag bit that says it is there.
    let (strings, data_end) = if layout < Layout::R2007 {
        (None, u64::from(header.obj_size))
    } else {
        let flag_bit = header
            .obj_size
            .checked_sub(1)
            .ok_or_else(|| format_error("MULTILEADER has no data"))?;
        let mut flag = reader.clone();
        flag.set_bit_pos(flag_bit);
        if flag.read_b()? == 0 {
            (None, u64::from(flag_bit))
        } else {
            let (strings, strings_end) = string_stream_reader(reader, header.obj_size)
                .ok_or_else(|| format_error("MULTILEADER string stream is not usable"))?;
            let data_end = strings.tell_bits();
            (Some((strings, u64::from(strings_end))), data_end)
        }
    };

    let mut walk = Walk {
        data: reader.clone(),
        data_end,
        strings,
        handles,
        base_handle: header.handle,
        layout,
        finite: true,
    };

    let class_version = if layout >= Layout::R2010 {
        Some(walk.short()?)
    } else {
        None
    };
    let context = walk.context()?;

    let style_handle = walk.handle()?;
    let override_flags = walk.long()?;
    let leader_line_type = walk.short()?;
    let leader_color = walk.color()?;
    let leader_linetype_handle = walk.handle()?;
    let leader_lineweight = walk.long()? as i32;
    let landing_enabled = walk.flag()?;
    let dogleg_enabled = walk.flag()?;
    let dogleg_length = walk.double()?;
    let arrow_handle = walk.handle()?;
    let arrow_size = walk.double()?;
    let content_type = walk.short()?;
    let text_style_handle = walk.handle()?;
    let left_attachment = walk.short()?;
    let right_attachment = walk.short()?;
    // The text angle type, the text alignment type and the text color: one
    // value each in all the evidence, or a copy of a context value.
    walk.short()?;
    walk.short()?;
    walk.color()?;
    let text_frame = walk.flag()?;
    let block_handle = walk.handle()?;
    // The block color, scale and rotation (copies of the context values).
    walk.color()?;
    walk.point()?;
    walk.double()?;
    let block_connection = walk.short()?;
    // The annotation scale flag: 0 in all the evidence.
    walk.flag()?;
    let attributes = walk.attributes()?;
    // A flag, two shorts and a scale factor; from R2010 the attachment
    // direction and the top and bottom attachment again; from R2013 one more
    // flag. Copies of values above, or one value in all the evidence.
    walk.flag()?;
    walk.short()?;
    walk.short()?;
    walk.double()?;
    if layout >= Layout::R2010 {
        walk.short()?;
        walk.short()?;
        walk.short()?;
    }
    if layout >= Layout::R2013 {
        walk.flag()?;
    }
    if !walk.ends_exact() {
        return Err(format_error(
            "MULTILEADER record does not end where its layout ends",
        ));
    }

    let entity_color = header.color;
    Ok(MLeaderEntity {
        handle: header.handle,
        owner_handle: common_handles.owner_ref,
        layer_handle: common_handles.layer,
        color_index: entity_color.index,
        true_color: entity_color.true_color,
        class_version,
        context,
        style_handle,
        override_flags,
        leader_line_type,
        leader_color,
        leader_linetype_handle,
        leader_lineweight,
        landing_enabled,
        dogleg_enabled,
        dogleg_length,
        arrow_handle,
        arrow_size,
        content_type,
        text_style_handle,
        left_attachment,
        right_attachment,
        text_frame,
        block_handle,
        block_connection,
        attributes,
        finite: walk.finite,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::{BitWriter, Endian};

    const HANDLE: u64 = 0x50;
    const LAYOUTS: [Layout; 5] = [
        Layout::R2000,
        Layout::R2004,
        Layout::R2007,
        Layout::R2010,
        Layout::R2013,
    ];

    /// What a test record holds. `lines` gives, for each leader, the number
    /// of points of each of its lines.
    #[derive(Clone)]
    struct Spec {
        layout: Layout,
        lines: Vec<Vec<usize>>,
        text: bool,
        block: bool,
        attributes: usize,
        owner: bool,
        // Counts of items whose layout is not known.
        leader_breaks: u32,
        line_breaks: u32,
        column_heights: u32,
        old_lists: (u32, u32),
        // A count the record states in place of the true one.
        claimed_leaders: Option<u32>,
        claimed_lines: Option<u32>,
        claimed_points: Option<u32>,
        claimed_attributes: Option<u32>,
        /// The double with this number is written with this value.
        bad_double: Option<(usize, f64)>,
        /// The record says it has a text label and holds no string for it.
        no_label_string: bool,
        /// Every color has a color name and a color book name.
        color_names: bool,
        // Content after the end of the layout.
        extra_data_bit: bool,
        extra_string: bool,
        extra_handle: bool,
    }

    impl Spec {
        fn text(layout: Layout) -> Self {
            Self {
                layout,
                lines: vec![vec![1]],
                text: true,
                block: false,
                attributes: 0,
                owner: false,
                leader_breaks: 0,
                line_breaks: 0,
                column_heights: 0,
                old_lists: (0, 0),
                claimed_leaders: None,
                claimed_lines: None,
                claimed_points: None,
                claimed_attributes: None,
                bad_double: None,
                no_label_string: false,
                color_names: false,
                extra_data_bit: false,
                extra_string: false,
                extra_handle: false,
            }
        }

        fn block(layout: Layout) -> Self {
            Self {
                text: false,
                block: true,
                attributes: usize::from(layout >= Layout::R2010),
                ..Self::text(layout)
            }
        }
    }

    struct Record {
        layout: Layout,
        bytes: Vec<u8>,
        /// The bit where the handle stream begins.
        data_end_bit: u32,
        /// What the record was written from.
        entity: MLeaderEntity,
        /// How many doubles the record holds.
        doubles: usize,
    }

    /// Writes a record field by field and keeps what it wrote.
    struct Builder {
        writer: BitWriter,
        spec: Spec,
        handles: Vec<u64>,
        doubles: usize,
        shorts: u16,
    }

    impl Builder {
        fn flag(&mut self, value: bool) -> bool {
            self.writer.write_b(u8::from(value)).expect("flag");
            value
        }

        /// A short of its own value, so that two fields never agree.
        fn short(&mut self) -> u16 {
            self.shorts += 1;
            self.writer.write_bs(self.shorts).expect("short");
            self.shorts
        }

        fn long(&mut self, value: u32) -> u32 {
            self.writer.write_bl(value).expect("long");
            value
        }

        /// A double of its own value.
        fn double(&mut self) -> f64 {
            let mut value = 200.5 + self.doubles as f64 * 1.25;
            if let Some((number, bad)) = self.spec.bad_double {
                if number == self.doubles {
                    value = bad;
                }
            }
            self.doubles += 1;
            // Not `write_bd`: it has no code for a value that is not finite.
            self.writer.write_bb(0).expect("double code");
            self.writer.write_rd(Endian::Little, value).expect("double");
            value
        }

        fn point(&mut self) -> Point3 {
            (self.double(), self.double(), self.double())
        }

        fn handle(&mut self) -> u64 {
            let value = 0x300 + self.handles.len() as u64;
            self.handles.push(value);
            value
        }

        fn text(&mut self, text: &str) -> String {
            self.writer.write_tv(text).expect("text");
            text.to_string()
        }

        fn color(&mut self) -> MLeaderColor {
            let index = self.short();
            if self.spec.layout == Layout::R2000 {
                return MLeaderColor { index, rgb: 0 };
            }
            let rgb = 0xC200_0000 | u32::from(index);
            self.writer.write_bl(rgb).expect("color value");
            if self.spec.color_names {
                self.writer.write_rc(3).expect("two color names");
                self.text("color name");
                self.text("book name");
            } else {
                self.writer.write_rc(0).expect("no color names");
            }
            MLeaderColor { index, rgb }
        }

        /// The common entity data from the handle on.
        fn common(&mut self, obj_size: u32) {
            let layout = self.spec.layout;
            if layout < Layout::R2010 {
                self.writer
                    .write_rl(Endian::Little, obj_size)
                    .expect("size");
            }
            self.writer.write_inline_h(0, HANDLE).expect("handle");
            self.writer.write_bs(0).expect("no extended data");
            self.writer.write_b(0).expect("no proxy graphic");
            let entity_mode = if self.spec.owner { 0 } else { 2 };
            self.writer.write_bb(entity_mode).expect("entity mode");
            self.writer.write_bl(0).expect("reactor count");
            // R2000: no links. Later: no extension dictionary.
            self.writer.write_b(1).expect("flag");
            if layout >= Layout::R2013 {
                self.writer.write_b(0).expect("no data store data");
            }
            self.writer.write_b(1).expect("color by layer");
            self.writer.write_b(0).expect("color");
            self.writer.write_bd(1.0).expect("linetype scale");
            self.writer.write_bb(0).expect("linetype flags");
            self.writer.write_bb(0).expect("plot style flags");
            if layout >= Layout::R2007 {
                self.writer.write_bb(0).expect("material flags");
                self.writer.write_rc(0).expect("shadow flags");
            }
            if layout >= Layout::R2010 {
                for _ in 0..3 {
                    self.writer.write_b(0).expect("visual style flag");
                }
            }
            self.writer.write_bs(0).expect("invisibility");
            self.writer.write_rc(0).expect("lineweight");
        }

        fn leaders(&mut self) -> Vec<MLeaderRoot> {
            let spec = self.spec.clone();
            let per_line_properties = spec.layout >= Layout::R2010;
            self.long(spec.claimed_leaders.unwrap_or(spec.lines.len() as u32));
            let mut leaders = Vec::new();
            for line_points in &spec.lines {
                let has_last_point = self.flag(true);
                let has_dogleg = self.flag(false);
                let last_point = self.point();
                let dogleg_vector = self.point();
                self.long(spec.leader_breaks);
                let index = self.long(leaders.len() as u32 + 40);
                let dogleg_length = self.double();
                self.long(spec.claimed_lines.unwrap_or(line_points.len() as u32));
                let mut lines = Vec::new();
                for point_count in line_points {
                    self.long(spec.claimed_points.unwrap_or(*point_count as u32));
                    let points = (0..*point_count).map(|_| self.point()).collect();
                    self.long(spec.line_breaks);
                    let index = self.long(lines.len() as u32 + 60);
                    if per_line_properties {
                        self.short();
                        self.color();
                        self.handle();
                        self.long(7);
                        self.double();
                        self.handle();
                        self.long(8);
                    }
                    lines.push(MLeaderLine { index, points });
                }
                let attachment_direction = per_line_properties.then(|| self.short());
                leaders.push(MLeaderRoot {
                    has_last_point,
                    has_dogleg,
                    last_point,
                    dogleg_vector,
                    index,
                    dogleg_length,
                    lines,
                    attachment_direction,
                });
            }
            leaders
        }

        fn text_content(&mut self) -> MLeaderText {
            let text = if self.spec.no_label_string {
                String::new()
            } else {
                self.text("label")
            };
            let normal = self.point();
            let style_handle = self.handle();
            let location = self.point();
            let direction = self.point();
            let rotation = self.double();
            let width = self.double();
            self.double();
            self.double();
            self.short();
            let color = self.color();
            let attachment = self.short();
            let drawing_direction = self.short();
            let background_color = self.color();
            let background_scale = self.double();
            self.long(9);
            let background_on = self.flag(true);
            self.flag(false);
            self.short();
            self.flag(true);
            self.double();
            self.double();
            self.flag(false);
            self.long(self.spec.column_heights);
            self.flag(true);
            MLeaderText {
                text,
                normal,
                style_handle,
                location,
                direction,
                rotation,
                width,
                color,
                attachment,
                drawing_direction,
                background_color,
                background_scale,
                background_on,
            }
        }

        fn block_content(&mut self) -> MLeaderBlock {
            let block_handle = self.handle();
            let normal = self.point();
            let location = self.point();
            let scale = self.point();
            let rotation = self.double();
            let color = self.color();
            let mut matrix = [0.0; 16];
            for value in &mut matrix {
                *value = self.double();
            }
            MLeaderBlock {
                block_handle,
                normal,
                location,
                scale,
                rotation,
                color,
                matrix,
            }
        }

        fn context(&mut self) -> MLeaderContext {
            let leaders = self.leaders();
            let scale = self.double();
            let content_base = self.point();
            let text_height = self.double();
            let arrow_size = self.double();
            let landing_gap = self.double();
            let left_attachment = self.short();
            let right_attachment = self.short();
            let text_alignment = self.short();
            let attachment_type = self.short();
            let text = self.flag(self.spec.text).then(|| self.text_content());
            let block = self.flag(self.spec.block).then(|| self.block_content());
            let plane_origin = self.point();
            let plane_x_axis = self.point();
            let plane_y_axis = self.point();
            let plane_normal_reversed = self.flag(true);
            if self.spec.layout >= Layout::R2010 {
                self.short();
                self.short();
            }
            MLeaderContext {
                leaders,
                scale,
                content_base,
                text_height,
                arrow_size,
                landing_gap,
                left_attachment,
                right_attachment,
                text_alignment,
                attachment_type,
                text,
                block,
                plane_origin,
                plane_x_axis,
                plane_y_axis,
                plane_normal_reversed,
            }
        }

        fn attributes(&mut self) -> Vec<MLeaderAttribute> {
            let spec = self.spec.clone();
            if spec.layout < Layout::R2010 {
                self.long(spec.old_lists.0);
                self.long(spec.old_lists.1);
                return Vec::new();
            }
            self.long(spec.claimed_attributes.unwrap_or(spec.attributes as u32));
            (0..spec.attributes)
                .map(|number| MLeaderAttribute {
                    definition_handle: self.handle(),
                    index: self.short(),
                    width: self.double(),
                    text: self.text(&format!("value {number}")),
                })
                .collect()
        }

        /// The whole record; `obj_size` is the bit where its handle stream
        /// begins, which the versions before R2010 state in the record.
        fn record(mut self, obj_size: u32) -> Record {
            let layout = self.spec.layout;
            // The common handles: the owner, the extension dictionary
            // (R2000 always has one) and the layer.
            let owner_handle = self.spec.owner.then(|| self.handle());
            if layout == Layout::R2000 {
                self.handle();
            }
            let layer_handle = self.handle();

            self.common(obj_size);
            let class_version = (layout >= Layout::R2010).then(|| self.short());
            let context = self.context();
            let style_handle = self.handle();
            let override_flags = self.long(0x0123_4567);
            let leader_line_type = self.short();
            let leader_color = self.color();
            let leader_linetype_handle = self.handle();
            let leader_lineweight = self.long(-2i32 as u32) as i32;
            let landing_enabled = self.flag(true);
            let dogleg_enabled = self.flag(false);
            let dogleg_length = self.double();
            let arrow_handle = self.handle();
            let arrow_size = self.double();
            let content_type = self.short();
            let text_style_handle = self.handle();
            let left_attachment = self.short();
            let right_attachment = self.short();
            self.short();
            self.short();
            self.color();
            let text_frame = self.flag(true);
            let block_handle = self.handle();
            self.color();
            self.point();
            self.double();
            let block_connection = self.short();
            self.flag(false);
            let attributes = self.attributes();
            self.flag(true);
            self.short();
            self.short();
            self.double();
            if layout >= Layout::R2010 {
                self.short();
                self.short();
                self.short();
            }
            if layout >= Layout::R2013 {
                self.flag(true);
            }
            if self.spec.extra_data_bit {
                self.flag(true);
            }
            if self.spec.extra_string {
                self.text("one more");
            }
            if self.spec.extra_handle {
                self.handle();
            }

            self.writer.finish_strings().expect("string stream");
            let data_end_bit = self.writer.tell_bits() as u32;
            for handle in &self.handles {
                self.writer.write_inline_h(5, *handle).expect("handle");
            }
            Record {
                layout,
                bytes: self.writer.into_bytes(),
                data_end_bit,
                entity: MLeaderEntity {
                    handle: HANDLE,
                    owner_handle,
                    layer_handle,
                    color_index: Some(0),
                    true_color: None,
                    class_version,
                    context,
                    style_handle,
                    override_flags,
                    leader_line_type,
                    leader_color,
                    leader_linetype_handle,
                    leader_lineweight,
                    landing_enabled,
                    dogleg_enabled,
                    dogleg_length,
                    arrow_handle,
                    arrow_size,
                    content_type,
                    text_style_handle,
                    left_attachment,
                    right_attachment,
                    text_frame,
                    block_handle,
                    block_connection,
                    attributes,
                    finite: self.spec.bad_double.is_none(),
                },
                doubles: self.doubles,
            }
        }
    }

    fn record(spec: &Spec) -> Record {
        let build = |obj_size| {
            Builder {
                writer: if spec.layout >= Layout::R2007 {
                    BitWriter::with_unicode()
                } else {
                    BitWriter::new()
                },
                spec: spec.clone(),
                handles: Vec::new(),
                doubles: 0,
                shorts: 0,
            }
            .record(obj_size)
        };
        // Written once to learn where the handle stream begins.
        let data_end_bit = build(0).data_end_bit;
        build(data_end_bit)
    }

    fn decode_bytes(layout: Layout, bytes: &[u8], data_end_bit: u32) -> Result<MLeaderEntity> {
        let mut reader = BitReader::new(bytes);
        match layout {
            Layout::R2000 => {
                reader.set_pre_r2004_layout(true);
                decode_mleader(&mut reader)
            }
            Layout::R2004 => decode_mleader(&mut reader),
            Layout::R2007 => decode_mleader_r2007(&mut reader),
            Layout::R2010 => decode_mleader_r2010(&mut reader, data_end_bit, HANDLE),
            Layout::R2013 => decode_mleader_r2013(&mut reader, data_end_bit, HANDLE),
        }
    }

    fn decode(record: &Record) -> Result<MLeaderEntity> {
        decode_bytes(record.layout, &record.bytes, record.data_end_bit)
    }

    fn error_of(spec: &Spec) -> DwgError {
        decode(&record(spec)).expect_err("the record is refused")
    }

    #[test]
    fn text_label_is_read_in_every_layout() {
        for layout in LAYOUTS {
            let record = record(&Spec::text(layout));
            let entity = decode(&record).expect("decodes");
            assert_eq!(entity, record.entity, "{layout:?}");
            assert_eq!(entity.class_version.is_some(), layout >= Layout::R2010);
            let text = entity.context.text.expect("a text");
            assert_eq!(text.text, "label");
            assert_eq!(text.color.rgb != 0, layout >= Layout::R2004);
            assert!(entity.context.block.is_none());
        }
    }

    #[test]
    fn block_label_is_read_in_every_layout() {
        for layout in LAYOUTS {
            let record = record(&Spec::block(layout));
            let entity = decode(&record).expect("decodes");
            assert_eq!(entity, record.entity, "{layout:?}");
            assert!(entity.context.text.is_none());
            let block = entity.context.block.expect("a block");
            assert_ne!(block.block_handle, entity.block_handle);
            assert_ne!(block.matrix[0], block.matrix[15]);
            let texts: Vec<&str> = entity.attributes.iter().map(|a| a.text.as_str()).collect();
            if layout >= Layout::R2010 {
                assert_eq!(texts, ["value 0"]);
            } else {
                assert!(texts.is_empty());
            }
        }
    }

    #[test]
    fn two_block_attributes_are_read() {
        let mut spec = Spec::block(Layout::R2013);
        spec.attributes = 2;
        let record = record(&spec);
        let entity = decode(&record).expect("decodes");
        assert_eq!(entity, record.entity);
        assert_eq!(entity.attributes.len(), 2);
        assert_eq!(entity.attributes[1].text, "value 1");
        assert_ne!(
            entity.attributes[0].definition_handle,
            entity.attributes[1].definition_handle
        );
    }

    #[test]
    fn label_of_no_content_is_read() {
        for layout in LAYOUTS {
            let mut spec = Spec::text(layout);
            spec.text = false;
            let record = record(&spec);
            let entity = decode(&record).expect("decodes");
            assert_eq!(entity, record.entity, "{layout:?}");
            assert!(entity.context.text.is_none() && entity.context.block.is_none());
        }
    }

    #[test]
    fn several_leaders_lines_and_points_are_read() {
        for layout in LAYOUTS {
            let mut spec = Spec::text(layout);
            spec.lines = vec![vec![3, 1], vec![2], vec![]];
            spec.owner = true;
            let record = record(&spec);
            let entity = decode(&record).expect("decodes");
            assert_eq!(entity, record.entity, "{layout:?}");
            let leaders = &entity.context.leaders;
            let points: Vec<Vec<usize>> = leaders
                .iter()
                .map(|leader| leader.lines.iter().map(|line| line.points.len()).collect())
                .collect();
            assert_eq!(points, spec.lines);
            assert_ne!(leaders[0].lines[0].points[0], leaders[0].lines[0].points[2]);
            assert_ne!(leaders[0].last_point, leaders[1].last_point);
            assert!(entity.owner_handle.is_some());
        }
    }

    #[test]
    fn color_names_are_stepped_over() {
        for layout in [Layout::R2004, Layout::R2007, Layout::R2010, Layout::R2013] {
            for mut spec in [Spec::text(layout), Spec::block(layout)] {
                spec.color_names = true;
                let record = record(&spec);
                assert_eq!(
                    decode(&record).expect("decodes"),
                    record.entity,
                    "{layout:?}"
                );
            }
        }
    }

    #[test]
    fn text_label_without_a_string_stream_is_refused() {
        for layout in [Layout::R2007, Layout::R2010, Layout::R2013] {
            let mut spec = Spec::text(layout);
            spec.no_label_string = true;
            assert_eq!(
                error_of(&spec).message,
                "MULTILEADER has a text field and no string stream",
                "{layout:?}"
            );
        }
    }

    #[test]
    fn count_larger_than_the_record_is_refused_before_its_items_are_read() {
        for layout in LAYOUTS {
            for claimed in [1_000_000, u32::MAX] {
                let mut cases = vec![
                    ("leader", Spec::text(layout)),
                    ("leader line", Spec::text(layout)),
                    ("leader line point", Spec::text(layout)),
                ];
                cases[0].1.claimed_leaders = Some(claimed);
                cases[1].1.claimed_lines = Some(claimed);
                cases[2].1.claimed_points = Some(claimed);
                if layout >= Layout::R2010 {
                    let mut spec = Spec::block(layout);
                    spec.claimed_attributes = Some(claimed);
                    cases.push(("block attribute", spec));
                }
                for (what, spec) in cases {
                    let error = error_of(&spec);
                    assert_eq!(error.kind, ErrorKind::Format, "{layout:?} {what}");
                    assert_eq!(
                        error.message,
                        format!("MULTILEADER {what} count {claimed} is larger than the record")
                    );
                }
            }
        }
    }

    /// A walk over bare streams, for the list readers.
    fn walk<'a>(
        layout: Layout,
        data: &'a [u8],
        data_end: u64,
        strings: &'a [u8],
        handles: &'a [u8],
    ) -> Walk<'a> {
        Walk {
            data: BitReader::new(data),
            data_end,
            strings: Some((BitReader::new(strings), strings.len() as u64 * 8)),
            handles: BitReader::new(handles),
            base_handle: HANDLE,
            layout,
            finite: true,
        }
    }

    /// Longs, shorts or doubles of the value 0: the short form, 2 bits each.
    fn zeros(writer: &mut BitWriter, count: usize) {
        for _ in 0..count {
            writer.write_bb(2).expect("zero");
        }
    }

    /// A leader before R2010 up to its line count: two flags off, the end
    /// of its lines and the dogleg vector (3 doubles each), no break, the
    /// index and the dogleg length.
    fn least_leader_head(writer: &mut BitWriter) {
        writer.write_b(0).expect("flag");
        writer.write_b(0).expect("flag");
        zeros(writer, 9);
    }

    const MANY: u32 = 40;

    fn count_error(what: &str) -> String {
        format!("MULTILEADER {what} count {MANY} is larger than the record")
    }

    #[test]
    fn list_of_least_items_is_read_with_its_count_exactly_at_the_bound() {
        // Every item is as small as the layout lets it be: its numbers are
        // 0 in the short form and its lists are empty. Such a list takes
        // exactly its bound in bits, so a bound that is one bit too high
        // would refuse a record that is whole. One bit less than the bound
        // is refused before memory is reserved.
        let many = MANY as usize;

        // Leaders without lines (a layout before R2010).
        let mut writer = BitWriter::new();
        writer.write_bl(MANY).expect("leader count");
        let begin = writer.tell_bits();
        for _ in 0..many {
            least_leader_head(&mut writer);
            zeros(&mut writer, 1);
        }
        let end = writer.tell_bits();
        assert_eq!(end - begin, u64::from(MANY) * LEADER_BITS);
        let data = writer.into_bytes();
        let leaders = walk(Layout::R2004, &data, end, &[], &[])
            .leaders()
            .expect("at the bound");
        assert_eq!(leaders.len(), many);
        assert!(leaders.iter().all(|leader| leader.lines.is_empty()));
        let error = walk(Layout::R2004, &data, end - 1, &[], &[])
            .leaders()
            .expect_err("one bit short");
        assert_eq!(error.message, count_error("leader"));

        // Lines without points: no point, no break, the index.
        let mut writer = BitWriter::new();
        writer.write_bl(1).expect("leader count");
        least_leader_head(&mut writer);
        writer.write_bl(MANY).expect("line count");
        let begin = writer.tell_bits();
        zeros(&mut writer, 3 * many);
        let end = writer.tell_bits();
        assert_eq!(end - begin, u64::from(MANY) * LINE_BITS);
        let data = writer.into_bytes();
        let leaders = walk(Layout::R2004, &data, end, &[], &[])
            .leaders()
            .expect("at the bound");
        assert_eq!(leaders[0].lines.len(), many);
        let error = walk(Layout::R2004, &data, end - 1, &[], &[])
            .leaders()
            .expect_err("one bit short");
        assert_eq!(error.message, count_error("leader line"));

        // Points at the origin. The data ends with the last point here: the
        // two fields that close the line are there, past that end.
        let mut writer = BitWriter::new();
        writer.write_bl(1).expect("leader count");
        least_leader_head(&mut writer);
        writer.write_bl(1).expect("line count");
        writer.write_bl(MANY).expect("count of points");
        let begin = writer.tell_bits();
        zeros(&mut writer, 3 * many);
        let end = writer.tell_bits();
        assert_eq!(end - begin, u64::from(MANY) * POINT_BITS);
        zeros(&mut writer, 2);
        let data = writer.into_bytes();
        let leaders = walk(Layout::R2004, &data, end, &[], &[])
            .leaders()
            .expect("at the bound");
        assert_eq!(leaders[0].lines[0].points, vec![(0.0, 0.0, 0.0); many]);
        let error = walk(Layout::R2004, &data, end - 1, &[], &[])
            .leaders()
            .expect_err("one bit short");
        assert_eq!(error.message, count_error("leader line point"));

        // Block attributes (R2010 and later): the index and the width in the
        // data, the definition in the handle stream, an empty text in the
        // string stream.
        let mut writer = BitWriter::new();
        writer.write_bl(MANY).expect("attribute count");
        let begin = writer.tell_bits();
        zeros(&mut writer, 2 * many);
        let end = writer.tell_bits();
        assert_eq!(end - begin, u64::from(MANY) * ATTRIBUTE_BITS);
        let data = writer.into_bytes();
        let mut strings = BitWriter::new();
        zeros(&mut strings, many);
        let strings = strings.into_bytes();
        let mut handles = BitWriter::new();
        for number in 0..u64::from(MANY) {
            handles.write_inline_h(5, 0x300 + number).expect("handle");
        }
        let handles = handles.into_bytes();
        let attributes = walk(Layout::R2010, &data, end, &strings, &handles)
            .attributes()
            .expect("at the bound");
        assert_eq!(attributes.len(), many);
        assert_eq!(
            attributes[many - 1].definition_handle,
            0x300 + u64::from(MANY) - 1
        );
        assert!(attributes.iter().all(|attribute| attribute.text.is_empty()));
        let error = walk(Layout::R2010, &data, end - 1, &strings, &handles)
            .attributes()
            .expect_err("one bit short");
        assert_eq!(error.message, count_error("block attribute"));
    }

    #[test]
    fn count_one_above_the_true_one_is_refused() {
        // The record states one item more than it holds; the bits that
        // follow are read as that item, and the walk does not end exact.
        for layout in LAYOUTS {
            let mut leaders = Spec::text(layout);
            leaders.claimed_leaders = Some(2);
            let mut lines = Spec::text(layout);
            lines.claimed_lines = Some(2);
            let mut points = Spec::text(layout);
            points.claimed_points = Some(2);
            for spec in [leaders, lines, points] {
                assert!(decode(&record(&spec)).is_err(), "{layout:?}");
            }
        }
    }

    #[test]
    fn item_of_an_unknown_layout_is_refused() {
        for layout in LAYOUTS {
            let mut cases = vec![Spec::text(layout), Spec::text(layout), Spec::text(layout)];
            cases[0].leader_breaks = 1;
            cases[1].line_breaks = 1;
            cases[2].column_heights = 1;
            if layout < Layout::R2010 {
                let mut first = Spec::text(layout);
                first.old_lists = (1, 0);
                let mut second = Spec::text(layout);
                second.old_lists = (0, 1);
                cases.extend([first, second]);
            }
            for spec in cases {
                let error = error_of(&spec);
                assert_eq!(error.kind, ErrorKind::Unsupported, "{layout:?}");
                assert!(error.message.contains("is not known"), "{}", error.message);
            }
        }
    }

    #[test]
    fn record_with_content_after_its_layout_is_refused() {
        for layout in LAYOUTS {
            let mut data = Spec::text(layout);
            data.extra_data_bit = true;
            let mut handle = Spec::text(layout);
            handle.extra_handle = true;
            let mut cases = vec![data, handle];
            if layout >= Layout::R2007 {
                let mut string = Spec::text(layout);
                string.extra_string = true;
                cases.push(string);
            }
            for spec in cases {
                assert_eq!(
                    error_of(&spec).message,
                    "MULTILEADER record does not end where its layout ends",
                    "{layout:?}"
                );
            }
        }
    }

    #[test]
    fn record_cut_short_is_an_error_at_every_length() {
        for layout in LAYOUTS {
            for spec in [Spec::text(layout), Spec::block(layout)] {
                let record = record(&spec);
                decode(&record).expect("whole");
                for len in 0..record.bytes.len() {
                    assert!(
                        decode_bytes(layout, &record.bytes[..len], record.data_end_bit).is_err(),
                        "{layout:?} cut at {len} of {}",
                        record.bytes.len()
                    );
                }
            }
        }
    }

    #[test]
    fn number_that_is_not_finite_is_reported() {
        for layout in [Layout::R2000, Layout::R2013] {
            for mut spec in [Spec::text(layout), Spec::block(layout)] {
                spec.lines = vec![vec![2]];
                let good = record(&spec);
                assert!(decode(&good).expect("decodes").has_finite_numbers());
                for number in 0..good.doubles {
                    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        spec.bad_double = Some((number, bad));
                        let entity = decode(&record(&spec)).expect("decodes");
                        assert!(!entity.has_finite_numbers(), "{layout:?} double {number}");
                    }
                }
            }
        }
    }
}
