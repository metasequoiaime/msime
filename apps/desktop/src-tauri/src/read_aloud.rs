//! 背单词卡片上的喇叭在 Linux 上怎么出声。
//!
//! macOS 和 Windows 的设置应用用网页自己的 `speechSynthesis`（只用本机声音），不经过这里。Linux 的 WebKitGTK 有没有这个接口取决于发行版的编译选项，所以和 IBus、Fcitx5 的候选菜单一样起 speech-dispatcher 自带的命令行 `spd-say`，不链接任何语音库；`PATH` 上没有它时 `HostCapabilities::read_aloud` 为假，卡片不画喇叭。见 `.agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md`。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// speech-dispatcher 自带的命令行。
const PROGRAM: &str = "spd-say";

/// `path`（`PATH` 的值）里第一个可执行的 `spd-say`。只认绝对目录：空的一段和相对目录指向当前目录，设置应用的当前目录不可信。
pub fn program_on_path(path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(PROGRAM))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// 本机能不能读：`PATH` 上有 `spd-say`。
pub fn available() -> bool {
    program_on_path(std::env::var_os("PATH").as_deref()).is_some()
}

/// 读出背单词卡片上的英文词头。只读共享层认作英文的那段（`vocabulary::read_aloud`），交给 `spd-say -l en` 就返回，不等它读完；另起一个线程收掉子进程。
#[tauri::command]
pub fn read_aloud(text: String) -> Result<(), String> {
    use msime_client_services::vocabulary::read_aloud::read_aloud_text;
    let text = read_aloud_text(&text, "").ok_or("只能朗读英文单词")?;
    let program = program_on_path(std::env::var_os("PATH").as_deref()).ok_or("没有找到 spd-say")?;
    let mut child = std::process::Command::new(program)
        .args(["-l", "en", &text])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "无法朗读".to_owned())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn spd_say_is_found_only_in_absolute_directories_and_only_when_executable() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let path = std::env::join_paths([Path::new("relative"), root.path(), &bin]).unwrap();
        assert_eq!(program_on_path(Some(&path)), None);
        let program = bin.join(PROGRAM);
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(program_on_path(Some(&path)), None, "not executable");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(program_on_path(Some(&path)), Some(program));
        assert_eq!(program_on_path(None), None);
    }

    #[test]
    fn only_english_is_read() {
        assert_eq!(read_aloud("面试".into()), Err("只能朗读英文单词".into()));
    }
}
