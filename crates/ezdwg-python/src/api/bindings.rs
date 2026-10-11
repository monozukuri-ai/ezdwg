#![allow(dead_code)] // recovery helpers retained for parity with upstream layer paths
#![allow(clippy::useless_conversion)] // Triggered by PyO3 #[pyfunction] wrapper expansion.

include!("bindings/shared.rs");
include!("bindings/cache.rs");
include!("bindings/write.rs");
include!("bindings/write_blocks.rs");
include!("bindings/write_polylines.rs");
include!("bindings/write_splines.rs");
include!("bindings/write_polyline_metadata.rs");
include!("bindings/write_metadata.rs");
include!("bindings/write_block_metadata.rs");
include!("bindings/decode.rs");
include!("bindings/layer.rs");
include!("bindings/dict_xrecord.rs");
include!("bindings/layer_states.rs");
include!("bindings/layer_filters.rs");
include!("bindings/linetype.rs");
include!("bindings/dimension.rs");
include!("bindings/polyline.rs");
include!("bindings/block_insert.rs");
include!("bindings/utils.rs");
include!("bindings/embedded_text.rs");
include!("bindings/register.rs");
