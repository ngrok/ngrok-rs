extern crate napi_build;

fn main() -> Result<(), &'static str> {
    // pyo3_build_config::add_extension_module_link_args();
    napi_build::setup();
    Ok(())
}
