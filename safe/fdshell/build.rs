#![allow(clippy::unwrap_used, clippy::expect_used)]

fn compile(name: &str) {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(name);
    let src = format!("tests/bins/{name}.rs");
    println!("cargo::rerun-if-changed={src}");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "-C",
            "opt-level=0",
            "-o",
            out.to_str().unwrap(),
            &src,
        ])
        .status()
        .expect("rustc not found");
    assert!(status.success(), "failed to compile {name}");
    println!(
        "cargo::rustc-env={}_PATH={}",
        name.to_uppercase(),
        out.display()
    );
}

fn main() {
    compile("exec_ok");
    compile("env_print");
    compile("arg_print");
}
