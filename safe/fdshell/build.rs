#![allow(clippy::unwrap_used, clippy::expect_used)]

fn main() {
    println!("cargo::rerun-if-changed=tests/bins/exec_ok.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let helper = out.join("exec_ok");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "-C",
            "opt-level=0",
            "-o",
            helper.to_str().unwrap(),
            "tests/bins/exec_ok.rs",
        ])
        .status()
        .expect("rustc not found");
    assert!(status.success(), "failed to compile exec_ok");
    println!("cargo::rustc-env=EXEC_OK_PATH={}", helper.display());
}
