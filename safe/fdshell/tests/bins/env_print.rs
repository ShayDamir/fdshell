fn main() {
    for (key, val) in std::env::vars() {
        println!("{}={}", key, val);
    }
    std::process::exit(0);
}
