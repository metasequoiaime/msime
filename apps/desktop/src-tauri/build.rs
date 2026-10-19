use std::{env, path::PathBuf};

fn link_ios_swift_runtime_exports() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    println!("cargo:rerun-if-env-changed=CONFIGURATION_BUILD_DIR");
    let Some(products) = env::var_os("CONFIGURATION_BUILD_DIR").map(PathBuf::from) else {
        return;
    };
    let archive = products.join("libMSIMESwiftRsRuntimeExports.a");
    if !archive.is_file() {
        panic!(
            "missing Xcode 27 SwiftRs runtime export archive: {}",
            archive.display()
        );
    }
    println!("cargo:rustc-link-search=native={}", products.display());
    println!("cargo:rustc-link-lib=static=MSIMESwiftRsRuntimeExports");
}

fn main() {
    link_ios_swift_runtime_exports();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=macos/settings_glass.mm");
        cc::Build::new()
            .cpp(true)
            .file("macos/settings_glass.mm")
            .flag("-fobjc-arc")
            .std("c++17")
            .compile("msime_settings_glass");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=WebKit");
    }
    tauri_build::build()
}
