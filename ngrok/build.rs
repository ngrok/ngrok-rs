extern crate napi_build;

fn main() -> Result<(), &'static str> {
    napi_build::setup();
    Ok(())
}
