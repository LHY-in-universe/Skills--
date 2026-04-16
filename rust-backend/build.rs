// sherpa-rs `download-binaries` 把 `libonnxruntime` / `libsherpa-onnx-c-api` 等 dylib
// 释放到 `target/<profile>/`，但不会给最终 binary 加 rpath。这里显式让 binary
// 在自身所在目录查找动态库（macOS @loader_path / Linux $ORIGIN），保证
// `cargo run` / `./target/release/skills-rust-backend` / 拷贝到部署位置都能起得来。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    } else {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
}
