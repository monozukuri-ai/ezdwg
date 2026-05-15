use pyo3::prelude::*;

mod api;

pub use ezdwg_core::{bit, container, core, dwg, entities, io, objects, writer};

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    api::register(m)
}
