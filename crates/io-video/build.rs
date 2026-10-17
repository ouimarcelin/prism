//! Compiles the Syphon framework sources (third_party/syphon, BSD licence)
//! and the Objective-C glue of Prism, on macOS.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/syphon");
    let syphon = root.join("Syphon");
    let mut build = cc::Build::new();
    for entry in std::fs::read_dir(&syphon).expect("third_party/syphon/Syphon") {
        let path = entry.expect("source file").path();
        if path.extension().is_some_and(|ext| ext == "m" || ext == "c") {
            build.file(&path);
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    build
        .file("src/syphon_glue.m")
        .include(&root)
        .include(&syphon)
        .flag("-fobjc-arc")
        .flag("-include")
        .flag(syphon.join("Syphon_Prefix.pch").to_str().expect("path"))
        .flag("-Wno-deprecated-declarations")
        .flag("-Wno-unused-parameter")
        .warnings(false)
        .compile("prism_syphon");
    println!("cargo:rerun-if-changed=src/syphon_glue.m");
    for framework in [
        "Foundation",
        "AppKit",
        "Metal",
        "IOSurface",
        "CoreFoundation",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
