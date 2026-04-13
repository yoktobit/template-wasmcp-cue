use std::env;
use std::path::PathBuf;

#[path = "../build-common.rs"]
#[allow(dead_code)]
mod build_common;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    build_common::run_component_codegen(
        &manifest_dir,
        "ACME_COMPONENT_PET_HANDLER",
        "acme:pet/api@0.1.0",
    );
}
