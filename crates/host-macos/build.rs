fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    println!("cargo:rerun-if-changed=native/keyboard.mm");
    println!("cargo:rerun-if-changed=native/keyboard.h");
    println!("cargo:rerun-if-changed=../../platforms/macos/src/input/ToggleHotkeyEvent.h");
    println!("cargo:rerun-if-changed=../../platforms/macos/src/input/ToggleHotkeys.h");
    println!(
        "cargo:rerun-if-changed=../../platforms/macos/src/core/ScreenKeyboardWindowAppearance.h"
    );
    println!("cargo:rerun-if-changed=../../platforms/macos/src/candidate/CandidateWindowLevel.h");
    println!("cargo:rerun-if-changed=native/voice_capture_devices.mm");
    println!("cargo:rerun-if-changed=native/clipboard.mm");
    println!("cargo:rerun-if-changed=native/uninstaller.mm");
    println!("cargo:rerun-if-changed=native/dictionary.mm");
    println!("cargo:rerun-if-changed=native/file_picker.mm");
    println!("cargo:rerun-if-changed=native/process.mm");
    // voice_capture_devices.mm includes this; the path moved with `refactor(macos): organize tests by
    // feature area` and the stale one meant an edit to the header rebuilt nothing.
    println!("cargo:rerun-if-changed=../../platforms/macos/src/voice/VoiceCaptureDevice.h");
    println!("cargo:rerun-if-changed=native/updater.mm");
    cc::Build::new()
        .cpp(true)
        .file("native/keyboard.mm")
        .file("native/voice_capture_devices.mm")
        .file("native/clipboard.mm")
        .file("native/uninstaller.mm")
        .file("native/dictionary.mm")
        .file("native/file_picker.mm")
        .file("native/process.mm")
        .file("native/updater.mm")
        .flag("-fobjc-arc")
        .std("c++17")
        .compile("msime_macos_keyboard");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=Security");
    // file_picker.mm 的应用选择面板只接受应用程序包（UTTypeApplicationBundle）。
    println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");
}
