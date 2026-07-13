#![forbid(unsafe_code)]

fn main() {
    let argv: Vec<std::ffi::CString> = std::env::args()
        .skip(1)
        .map(std::ffi::CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("pipe: argv contains NUL");
            std::process::exit(1);
        });
    let args: Vec<&core::ffi::CStr> = argv.iter().map(|cs| cs.as_c_str()).collect();

    let cfg = match builtins::pipe::parse::pipe_parse(&args) {
        Err(ref e) if matches!(e.current_context(), builtins::error::BuiltinError::Help) => {
            println!("Usage: pipe [--flags FLAGS]");
            return;
        }
        Err(e) => {
            eprintln!("pipe: parse error {e}");
            std::process::exit(1);
        }
        Ok(c) => c,
    };
    let sock_str = std::env::var("FDSHELL_SOCKET").unwrap_or_else(|_| {
        eprintln!("pipe: FDSHELL_SOCKET not set");
        std::process::exit(1);
    });
    let sock = match sys::ImportedFd::from_bytes(sock_str.as_bytes()) {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("pipe: invalid FDSHELL_SOCKET '{sock_str}': {e}");
            std::process::exit(1);
        }
    };
    let sock = match sock.try_into_local() {
        Ok(local) => local,
        Err(e) => {
            eprintln!("pipe: cannot lock shell socket: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = builtins::pipe::pipe_exec(cfg.flags, &sock) {
        eprintln!("pipe: error {e}");
        std::process::exit(1);
    }
}
