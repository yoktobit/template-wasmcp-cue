use std::path::PathBuf;
use std::process::Command;

fn run_cue(manifest_dir: &PathBuf, expr: &str, output_file: &str) {
    let status = Command::new("mise")
        .current_dir(manifest_dir)
        .args([
            "x",
            "--",
            "cue",
            "def",
            "-f",
            "schema.cue",
            "-e",
            expr,
            "-o",
            &format!("jsonschema:{output_file}"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cue`: {e}"));

    assert!(
        status.success(),
        "`cue def` failed while generating {output_file}. Is `cue` installed and on PATH?"
    );
}

fn main() {
    println!("cargo:rerun-if-changed={}", "schema.cue");

    run_cue(&PathBuf::from("."), "Input", "_input.schema.json");
    run_cue(&PathBuf::from("."), "Output", "_output.schema.json");
}
