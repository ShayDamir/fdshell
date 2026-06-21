#![forbid(unsafe_code)]

fn main() {
    let argv: Vec<std::ffi::CString> = std::env::args()
        .skip(1)
        .map(std::ffi::CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("renameat2: argv contains NUL");
            std::process::exit(1);
        });
    let args: Vec<&core::ffi::CStr> = argv.iter().map(|cs| cs.as_c_str()).collect();
    let cfg = match builtins::renameat2::parse::renameat2_parse(&args) {
        Err(builtins::error::BuiltinError::Help) => {
            println!(
                "Usage: renameat2 [--olddirfd N] [--newdirfd N] [--flags FLAGS] oldpath newpath"
            );
            return;
        }
        Err(e) => {
            eprintln!("renameat2: parse error {e}");
            std::process::exit(1);
        }
        Ok(c) => c,
    };
    if let Err(e) = builtins::renameat2::renameat2_exec(&cfg) {
        eprintln!("renameat2: error {e}");
        std::process::exit(1);
    }
}
