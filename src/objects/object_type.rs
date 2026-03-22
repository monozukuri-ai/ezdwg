#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectClass {
    Unused,
    Object,
    Entity,
}

impl ObjectClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unused => "",
            Self::Object => "O",
            Self::Entity => "E",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectTypeInfo {
    pub code: u16,
    pub name: &'static str,
    pub class: ObjectClass,
}

pub fn object_type_info(code: u16) -> ObjectTypeInfo {
    match code {
        0x00 => ObjectTypeInfo {
            code,
            name: "UNUSED",
            class: ObjectClass::Unused,
        },
        0x01 => ObjectTypeInfo {
            code,
            name: "TEXT",
            class: ObjectClass::Entity,
        },
        0x02 => ObjectTypeInfo {
            code,
            name: "ATTRIB",
            class: ObjectClass::Entity,
        },
        0x03 => ObjectTypeInfo {
            code,
            name: "ATTDEF",
            class: ObjectClass::Entity,
        },
        0x04 => ObjectTypeInfo {
            code,
            name: "BLOCK",
            class: ObjectClass::Entity,
        },
        0x05 => ObjectTypeInfo {
            code,
            name: "ENDBLK",
            class: ObjectClass::Entity,
        },
        0x06 => ObjectTypeInfo {
            code,
            name: "SEQEND",
            class: ObjectClass::Entity,
        },
        0x07 => ObjectTypeInfo {
            code,
            name: "INSERT",
            class: ObjectClass::Entity,
        },
        0x08 => ObjectTypeInfo {
            code,
            name: "MINSERT",
            class: ObjectClass::Entity,
        },
        0x0A => ObjectTypeInfo {
            code,
            name: "VERTEX_2D",
            class: ObjectClass::Entity,
        },
        0x0B => ObjectTypeInfo {
            code,
            name: "VERTEX_3D",
            class: ObjectClass::Entity,
        },
        0x0C => ObjectTypeInfo {
            code,
            name: "VERTEX_MESH",
            class: ObjectClass::Entity,
        },
        0x0D => ObjectTypeInfo {
            code,
            name: "VERTEX_PFACE",
            class: ObjectClass::Entity,
        },
        0x0E => ObjectTypeInfo {
            code,
            name: "VERTEX_PFACE_FACE",
            class: ObjectClass::Entity,
        },
        0x0F => ObjectTypeInfo {
            code,
            name: "POLYLINE_2D",
            class: ObjectClass::Entity,
        },
        0x10 => ObjectTypeInfo {
            code,
            name: "POLYLINE_3D",
            class: ObjectClass::Entity,
        },
        0x11 => ObjectTypeInfo {
            code,
            name: "ARC",
            class: ObjectClass::Entity,
        },
        0x12 => ObjectTypeInfo {
            code,
            name: "CIRCLE",
            class: ObjectClass::Entity,
        },
        0x13 => ObjectTypeInfo {
            code,
            name: "LINE",
            class: ObjectClass::Entity,
        },
        0x14 => ObjectTypeInfo {
            code,
            name: "DIM_ORDINATE",
            class: ObjectClass::Entity,
        },
        0x15 => ObjectTypeInfo {
            code,
            name: "DIM_LINEAR",
            class: ObjectClass::Entity,
        },
        0x16 => ObjectTypeInfo {
            code,
            name: "DIM_ALIGNED",
            class: ObjectClass::Entity,
        },
        0x17 => ObjectTypeInfo {
            code,
            name: "DIM_ANG3PT",
            class: ObjectClass::Entity,
        },
        0x18 => ObjectTypeInfo {
            code,
            name: "DIM_ANG2LN",
            class: ObjectClass::Entity,
        },
        0x19 => ObjectTypeInfo {
            code,
            name: "DIM_RADIUS",
            class: ObjectClass::Entity,
        },
        0x1A => ObjectTypeInfo {
            code,
            name: "DIM_DIAMETER",
            class: ObjectClass::Entity,
        },
        0x1B => ObjectTypeInfo {
            code,
            name: "POINT",
            class: ObjectClass::Entity,
        },
        0x1C => ObjectTypeInfo {
            code,
            name: "3DFACE",
            class: ObjectClass::Entity,
        },
        0x1D => ObjectTypeInfo {
            code,
            name: "POLYLINE_PFACE",
            class: ObjectClass::Entity,
        },
        0x1E => ObjectTypeInfo {
            code,
            name: "POLYLINE_MESH",
            class: ObjectClass::Entity,
        },
        0x1F => ObjectTypeInfo {
            code,
            name: "SOLID",
            class: ObjectClass::Entity,
        },
        0x20 => ObjectTypeInfo {
            code,
            name: "TRACE",
            class: ObjectClass::Entity,
        },
        0x21 => ObjectTypeInfo {
            code,
            name: "SHAPE",
            class: ObjectClass::Entity,
        },
        0x22 => ObjectTypeInfo {
            code,
            name: "VIEWPORT",
            class: ObjectClass::Entity,
        },
        0x23 => ObjectTypeInfo {
            code,
            name: "ELLIPSE",
            class: ObjectClass::Entity,
        },
        0x24 => ObjectTypeInfo {
            code,
            name: "SPLINE",
            class: ObjectClass::Entity,
        },
        0x25 => ObjectTypeInfo {
            code,
            name: "REGION",
            class: ObjectClass::Entity,
        },
        0x26 => ObjectTypeInfo {
            code,
            name: "3DSOLID",
            class: ObjectClass::Entity,
        },
        0x27 => ObjectTypeInfo {
            code,
            name: "BODY",
            class: ObjectClass::Entity,
        },
        0x28 => ObjectTypeInfo {
            code,
            name: "RAY",
            class: ObjectClass::Entity,
        },
        0x29 => ObjectTypeInfo {
            code,
            name: "XLINE",
            class: ObjectClass::Entity,
        },
        0x2A => ObjectTypeInfo {
            code,
            name: "DICTIONARY",
            class: ObjectClass::Object,
        },
        0x2B => ObjectTypeInfo {
            code,
            name: "OLEFRAME",
            class: ObjectClass::Entity,
        },
        0x2C => ObjectTypeInfo {
            code,
            name: "MTEXT",
            class: ObjectClass::Entity,
        },
        0x2D => ObjectTypeInfo {
            code,
            name: "LEADER",
            class: ObjectClass::Entity,
        },
        0x2E => ObjectTypeInfo {
            code,
            name: "TOLERANCE",
            class: ObjectClass::Entity,
        },
        0x2F => ObjectTypeInfo {
            code,
            name: "MLINE",
            class: ObjectClass::Entity,
        },
        0x30 => ObjectTypeInfo {
            code,
            name: "BLOCK_CONTROL",
            class: ObjectClass::Object,
        },
        0x31 => ObjectTypeInfo {
            code,
            name: "BLOCK_HEADER",
            class: ObjectClass::Object,
        },
        0x32 => ObjectTypeInfo {
            code,
            name: "LAYER_CONTROL",
            class: ObjectClass::Object,
        },
        0x33 => ObjectTypeInfo {
            code,
            name: "LAYER",
            class: ObjectClass::Object,
        },
        0x34 => ObjectTypeInfo {
            code,
            name: "SHAPEFILE_CONTROL",
            class: ObjectClass::Object,
        },
        0x35 => ObjectTypeInfo {
            code,
            name: "SHAPEFILE",
            class: ObjectClass::Object,
        },
        0x38 => ObjectTypeInfo {
            code,
            name: "LTYPE_CONTROL",
            class: ObjectClass::Object,
        },
        0x39 => ObjectTypeInfo {
            code,
            name: "LTYPE",
            class: ObjectClass::Object,
        },
        0x3C => ObjectTypeInfo {
            code,
            name: "VIEW_CONTROL",
            class: ObjectClass::Object,
        },
        0x3D => ObjectTypeInfo {
            code,
            name: "VIEW",
            class: ObjectClass::Object,
        },
        0x3E => ObjectTypeInfo {
            code,
            name: "UCS_CONTROL",
            class: ObjectClass::Object,
        },
        0x3F => ObjectTypeInfo {
            code,
            name: "UCS",
            class: ObjectClass::Object,
        },
        0x40 => ObjectTypeInfo {
            code,
            name: "VPORT_CONTROL",
            class: ObjectClass::Object,
        },
        0x41 => ObjectTypeInfo {
            code,
            name: "VPORT",
            class: ObjectClass::Object,
        },
        0x42 => ObjectTypeInfo {
            code,
            name: "APPID_CONTROL",
            class: ObjectClass::Object,
        },
        0x43 => ObjectTypeInfo {
            code,
            name: "APPID",
            class: ObjectClass::Object,
        },
        0x44 => ObjectTypeInfo {
            code,
            name: "DIMSTYLE_CONTROL",
            class: ObjectClass::Object,
        },
        0x45 => ObjectTypeInfo {
            code,
            name: "DIMSTYLE",
            class: ObjectClass::Object,
        },
        0x46 => ObjectTypeInfo {
            code,
            name: "VP_ENT_HDR_CONTROL",
            class: ObjectClass::Object,
        },
        0x47 => ObjectTypeInfo {
            code,
            name: "VP_ENT_HDR",
            class: ObjectClass::Object,
        },
        0x48 => ObjectTypeInfo {
            code,
            name: "GROUP",
            class: ObjectClass::Object,
        },
        0x49 => ObjectTypeInfo {
            code,
            name: "MLINESTYLE",
            class: ObjectClass::Object,
        },
        0x4A => ObjectTypeInfo {
            code,
            name: "OLE2FRAME",
            class: ObjectClass::Entity,
        },
        0x4C => ObjectTypeInfo {
            code,
            name: "LONG_TRANSACTION",
            class: ObjectClass::Entity,
        },
        0x4D => ObjectTypeInfo {
            code,
            name: "LWPOLYLINE",
            class: ObjectClass::Entity,
        },
        0x4E => ObjectTypeInfo {
            code,
            name: "HATCH",
            class: ObjectClass::Entity,
        },
        0x4F => ObjectTypeInfo {
            code,
            name: "XRECORD",
            class: ObjectClass::Object,
        },
        0x50 => ObjectTypeInfo {
            code,
            name: "ACDBPLACEHOLDER",
            class: ObjectClass::Object,
        },
        0x51 => ObjectTypeInfo {
            code,
            name: "VBA_PROJECT",
            class: ObjectClass::Object,
        },
        0x52 => ObjectTypeInfo {
            code,
            name: "LAYOUT",
            class: ObjectClass::Object,
        },
        0x1F2 => ObjectTypeInfo {
            code,
            name: "ACAD_PROXY_ENTITY",
            class: ObjectClass::Entity,
        },
        0x1F3 => ObjectTypeInfo {
            code,
            name: "ACAD_PROXY_OBJECT",
            class: ObjectClass::Object,
        },
        _ => ObjectTypeInfo {
            code,
            name: "UNKNOWN",
            class: ObjectClass::Unused,
        },
    }
}

pub fn object_type_name(code: u16) -> String {
    let info = object_type_info(code);
    if info.name == "UNKNOWN" {
        format!("UNKNOWN(0x{code:02X})")
    } else {
        info.name.to_string()
    }
}

pub fn object_type_class(code: u16) -> ObjectClass {
    object_type_info(code).class
}
