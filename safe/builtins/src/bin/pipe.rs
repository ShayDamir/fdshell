#![forbid(unsafe_code)]

fn main() {
    let argv: Vec<std::ffi::CString> = std::env::args().skip(1)
        .map(std::ffi::CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| { eprintln!("pipe: argv contains NUL"); std::process::exit(1); });
    let args: Vec<&core::ffi::CStr> = argv.iter().map(|cs| cs.as_c_str()).collect();

    match builtins::pipe::parse::pipe_parse(&args) {
        Err(0) => return,
        Err(e) => {
            eprintln!("pipe: parse error {e}");
            std::process::exit(1);
        }
        Ok(_) => {}
    };
    if let Err(e) = builtins::pipe::pipe_exec() {
        eprintln!("pipe: error {e}");
        std::process::exit(1);
    }
}
