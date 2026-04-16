fn main() {
    // Experiment mode: keep `wit/world.wit` as source-of-truth for component-pet.
    println!("cargo:rerun-if-changed=wit/world.wit");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
