#![forbid(unsafe_code)]

fn main() {
    let argv: Vec<std::ffi::CString> = std::env::args()
        .skip(1)
        .map(std::ffi::CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("openat2: argv contains NUL");
            std::process::exit(1);
        });
    let args: Vec<&core::ffi::CStr> = argv.iter().map(|cs| cs.as_c_str()).collect();

    let cfg = match builtins::openat2::parse::openat2_parse(&args) {
        Err(ref e) if matches!(e.current_context(), builtins::error::BuiltinError::Help) => {
            println!(
                "Usage: openat2 [--dirfd N] [--mode MODE] [--resolve FLAGS] [--flags FLAGS] path"
            );
            return;
        }
        Err(e) => {
            eprintln!("openat2: parse error {e}");
            std::process::exit(1);
        }
        Ok(c) => c,
    };
    if let Err(e) = builtins::openat2::openat2_exec(&cfg) {
        eprintln!("openat2: error {e}");
        std::process::exit(1);
    }
}
