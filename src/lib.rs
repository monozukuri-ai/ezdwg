use pyo3::prelude::*;

mod api;
pub mod bit;
pub mod container;
pub mod core;
pub mod dwg;
pub mod entities;
pub mod io;
pub mod objects;
pub mod writer;

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    api::register(m)
}
