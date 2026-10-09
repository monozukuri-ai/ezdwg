pub mod dictionary;
pub mod handle;
pub mod object_header_r2000;
pub mod object_header_r2010;
pub mod object_locator;
pub mod object_record;
pub mod object_ref;
pub mod object_type;
pub mod xrecord;

pub use dictionary::{
    assemble_dictionary, decode_common_object_handle_refs, decode_common_object_handles,
    decode_dictionary, decode_dictionary_data, decode_dictionary_handles, resolve_handle_ref,
    version_at_least, CommonObjectHandles, Dictionary, DictionaryData, DictionaryDecodeCtx,
    DictionaryEntry, DictionaryHandles,
};
pub use handle::Handle;
pub use object_header_r2000::{parse_at as parse_object_header_r2000, ObjectHeaderR2000};
pub use object_header_r2010::{parse_at as parse_object_header_r2010, ObjectHeaderR2010};
pub use object_locator::{build_object_index, build_object_index_from_directory, ObjectIndex};
pub use object_record::{parse_object_record, parse_object_record_r2010, ObjectRecord};
pub use object_ref::ObjectRef;
pub use object_type::{
    object_type_class, object_type_info, object_type_name, ObjectClass, ObjectTypeInfo,
};
pub use xrecord::{
    assemble_xrecord, decode_xrecord, decode_xrecord_data, decode_xrecord_data_with_end,
    decode_xrecord_handles, parse_xdata_groups, parse_xdata_groups_detailed,
    parse_xdata_groups_versioned, XDataGroup, XDataParseResult, XDataParseStop, XDataValue,
    XRecord, XRecordData, XRecordDecodeCtx, XRecordHandles,
};
