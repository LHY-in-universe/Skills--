// sherpa-rs `download-binaries` 把 `libonnxruntime` / `libsherpa-onnx-c-api` 等 dylib
// 释放到 `target/<profile>/`，但不会给最终 binary 加 rpath。这里显式让 binary
// 在自身所在目录查找动态库（macOS @loader_path / Linux $ORIGIN），保证
// `cargo run` / `./target/release/skills-rust-backend` / 拷贝到部署位置都能起得来。
fn main() {
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing");
    let profile_dir = std::path::Path::new(&manifest_dir)
        .join(target_dir)
        .join(profile);

    // sherpa-rs 在部分环境会把 dylib 复制到 target/<profile>/ 或 deps/，
    // 但上游给出的 link-search 目录可能失效。这里把最终落盘目录也显式加入。
    println!("cargo:rustc-link-search=native={}", profile_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        profile_dir.join("deps").display()
    );

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    } else {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
}
