#![forbid(unsafe_code)]

fn main() {
    let argv: Vec<std::ffi::CString> = std::env::args()
        .skip(1)
        .map(std::ffi::CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("mkdirat: argv contains NUL");
            std::process::exit(1);
        });
    let args: Vec<&core::ffi::CStr> = argv.iter().map(|cs| cs.as_c_str()).collect();

    let cfg = match builtins::mkdirat::parse::mkdirat_parse(&args) {
        Err(ref e) if matches!(e.current_context(), builtins::error::BuiltinError::Help) => {
            println!("Usage: mkdirat [--dirfd N] [--mode MODE] [--resolve FLAGS] path");
            return;
        }
        Err(e) => {
            eprintln!("mkdirat: parse error {e}");
            std::process::exit(1);
        }
        Ok(c) => c,
    };
    let sock_str = std::env::var("FDSHELL_SOCKET").unwrap_or_else(|_| {
        eprintln!("mkdirat: FDSHELL_SOCKET not set");
        std::process::exit(1);
    });
    let sock = match sys::ImportedFd::from_bytes(sock_str.as_bytes()) {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("mkdirat: invalid FDSHELL_SOCKET '{sock_str}': {e}");
            std::process::exit(1);
        }
    };
    let sock = match sock.try_into_local() {
        Ok(local) => local,
        Err(e) => {
            eprintln!("mkdirat: cannot lock shell socket: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = builtins::mkdirat::mkdirat_exec(&cfg, &sock) {
        eprintln!("mkdirat: error {e}");
        std::process::exit(1);
    }
}
