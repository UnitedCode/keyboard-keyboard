fn main() {
    let memory = std::path::Path::new("../../../../keyboard_keyboard/code")
        .canonicalize()
        .unwrap();
    println!("cargo:rustc-link-search={}", memory.display());
    println!("cargo:rerun-if-changed={}/memory.x", memory.display());
}
