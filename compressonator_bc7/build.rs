use std::{env, path::PathBuf};

fn main() {
    let root = env::var("COMPRESSONATOR_ROOT").unwrap();
    let mut lib_path = PathBuf::from(root)
        .canonicalize()
        .unwrap()
        .join("lib")
        .join("VS2017");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    match arch.as_str() {
        "x86_64" => lib_path.push("x64"),
        "x86" => lib_path.push("x86"),
        _ => panic!("incompatible target arch, must be 'x86_64' or 'x86'"),
    }
    println!("cargo:rustc-link-search=native={}", lib_path.display());

    let linkage = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    if linkage.contains("crt-static") {
        println!("cargo:rustc-link-lib=static=CMP_Core:CMP_Core_MT");
    } else {
        println!("cargo:rustc-link-lib=static=CMP_Core:CMP_Core_MD");
    }
}
