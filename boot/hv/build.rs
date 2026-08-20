use std::env;

fn main() {
    println!("cargo::rerun-if-env-changed=CARGO_FEATURE_QEMU");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rustc-link-search={}", manifest_dir);

    if env::var("CARGO_FEATURE_QEMU").is_ok() {
        println!("cargo::rustc-link-arg=-Tboot/linker-qemu.ld");
    } else {
        println!("cargo::rustc-link-arg=-Tboot/linker.ld");
    }
}
