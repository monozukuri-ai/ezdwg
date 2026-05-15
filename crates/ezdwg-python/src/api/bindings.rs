#![allow(clippy::useless_conversion)] // Triggered by PyO3 #[pyfunction] wrapper expansion.

include!("bindings/shared.rs");
include!("bindings/write.rs");
include!("bindings/decode.rs");
include!("bindings/layer.rs");
include!("bindings/dimension.rs");
include!("bindings/polyline.rs");
include!("bindings/block_insert.rs");
include!("bindings/utils.rs");
include!("bindings/register.rs");
