fn main() {
    // Keep `wit/world.wit` as source-of-truth for acme-component.
    println!("cargo:rerun-if-changed=wit/world.wit");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
