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
        Err(0) => return,
        Err(e) => {
            eprintln!("mkdirat: parse error {e}");
            std::process::exit(1);
        }
        Ok(c) => c,
    };
    if let Err(e) = builtins::mkdirat::mkdirat_exec(&cfg) {
        eprintln!("mkdirat: error {e}");
        std::process::exit(1);
    }
}
