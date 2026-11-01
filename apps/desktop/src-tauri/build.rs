use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

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

/// 关于页写在版本号后面的发布日期 `YYYY-MM-DD`，经 `MSIME_APP_RELEASE_DATE` 编进设置应用（`app_release_date` 命令）。
///
/// 来源依次是：发布脚本显式给的 `MSIME_RELEASE_DATE`（容器里的构建没有 `.git`，由宿主机上的脚本算好传进去）；源码包里所编平台的 `platforms/<os>/release-date.txt`，由 `platforms/linux/packaging/make-source-tarballs.sh` 这类生成源码包的脚本写入，仓库里没有这个文件；构建时源码树 HEAD 的提交日期（`git log -1 --format=%cs`），发布 workflow 从 main 上的发版合并提交构建，这就是发版当天。都没有时为空字符串，页面只显示版本号。
fn bake_release_date() {
    println!("cargo:rerun-if-env-changed=MSIME_RELEASE_DATE");
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("../..");
    let date = explicit_release_date()
        .or_else(|| release_date_file(&root))
        .or_else(|| git_commit_date(&root))
        .unwrap_or_default();
    println!("cargo:rustc-env=MSIME_APP_RELEASE_DATE={date}");
}

/// 与设置页 `parseReleaseDate` 认的形状相同：四位年、01–12 月、01–31 日。
fn is_release_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    let shaped = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    shaped
        && (1..=12).contains(&value[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&value[8..10].parse::<u8>().unwrap_or(0))
}

/// 显式给出的日期写错了就让构建失败：它只来自发布脚本和发布 workflow，悄悄丢掉会让发布包少了日期也没人发现。空值当作没给。
fn explicit_release_date() -> Option<String> {
    let value = env::var("MSIME_RELEASE_DATE").ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    assert!(
        is_release_date(value),
        "MSIME_RELEASE_DATE must be YYYY-MM-DD, not {value:?}"
    );
    Some(value.to_owned())
}

/// 源码包里所编平台的 `release-date.txt`，平台与 `version.txt` 的取法一致。只有三个桌面平台会有它。
fn release_date_file(root: &Path) -> Option<String> {
    let platform = match env::var("CARGO_CFG_TARGET_OS").ok()?.as_str() {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let file = root
        .join("platforms")
        .join(platform)
        .join("release-date.txt");
    let text = fs::read_to_string(&file).ok()?;
    // 只在文件存在时登记：登记一个不存在的文件会让 cargo 每次都重跑这个脚本。
    println!("cargo:rerun-if-changed={}", file.display());
    let value = text.trim();
    assert!(
        is_release_date(value),
        "{} must hold YYYY-MM-DD, not {value:?}",
        file.display()
    );
    Some(value.to_owned())
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

/// 源码树 HEAD 的提交日期。没有 git、不在仓库里、或者容器里的 git 不认这个检出（worktree 的 `.git` 指向宿主机路径、目录属主不同）时为 None。
fn git_commit_date(root: &Path) -> Option<String> {
    // 只认这个源码树自己的仓库：解包进另一个 git 检出的源码（例如 AUR 包仓库下的 src/）会向上找到那个仓库，它的提交日期与这次发布无关。
    let toplevel = git(root, &["rev-parse", "--show-toplevel"])?;
    if fs::canonicalize(toplevel).ok()? != fs::canonicalize(root).ok()? {
        return None;
    }
    // 提交、切换分支和 reset 都会改写 HEAD、它的 reflog 或它指向的分支文件，登记这几个，日期就不会过期；文件内容不动时 cargo 不重跑这个脚本。
    watch_git_path(root, "HEAD");
    watch_git_path(root, "logs/HEAD");
    if let Some(branch) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        watch_git_path(root, &branch);
    }
    let date = git(root, &["log", "-1", "--format=%cs"])?;
    is_release_date(&date).then_some(date)
}

fn watch_git_path(root: &Path, name: &str) {
    let Some(path) = git(root, &["rev-parse", "--git-path", name]) else {
        return;
    };
    // `--git-path` 给出相对 `-C` 目录的路径或绝对路径，join 对两者都成立。分支被打包进 packed-refs 时没有单独的文件，不登记。
    let path = root.join(path);
    if path.is_file() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn main() {
    link_ios_swift_runtime_exports();
    if matches!(
        env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("macos" | "windows" | "linux")
    ) {
        println!("cargo:rerun-if-changed=../../../platforms/common/CrashRecorder.cpp");
        println!("cargo:rerun-if-changed=../../../platforms/common/CrashRecorder.h");
        println!("cargo:rerun-if-changed=../../../platforms/common/PosixCrashFile.h");
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .file("../../../platforms/common/CrashRecorder.cpp")
            .compile("msime_native_crash");
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
            println!("cargo:rustc-link-lib=dl");
        }
    }
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
        println!("cargo:rustc-link-lib=framework=IOKit");
    }
    bake_release_date();
    tauri_build::build()
}
