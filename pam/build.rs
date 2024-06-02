use std::env;

fn main() {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if os != "linux" {
        panic!("Only Linux is supported");
    }
    let name = env::var_os("CARGO_PKG_NAME").unwrap();
    let name = name.to_str().expect("pkg name is not valid UTF-8");
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();

    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{}.so.{}", name, major);
}