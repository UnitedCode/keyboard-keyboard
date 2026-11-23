fn main() {
    let files = [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "src/main.rs",
        "src/noise.rs",
        "../../keyboard_keyboard/code/src/constants.rs",
        "../../keyboard_keyboard/code/src/switch.rs",
        "../../keyboard_keyboard/code/src/velocity.rs",
        "../../keyboard_keyboard/code/src/main.rs",
    ];
    let mut hashes = Vec::new();
    for file in files {
        println!("cargo:rerun-if-changed={file}");
        let r = std::process::Command::new("shasum")
            .args(["-a", "256", file])
            .output()
            .expect("shasum required");
        assert!(r.status.success());
        hashes.push(format!(
            "{file}={}",
            String::from_utf8(r.stdout)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
        ));
    }
    println!(
        "cargo:rustc-env=KEYBOARD_NOISE_SOURCES={}",
        hashes.join(";")
    );
}
