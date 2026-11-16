//! The root half of updating an input method installed in `/Library/Input Methods`.
//!
//! macOS 只把 `/Library/Input Methods` 里的第三方输入法算作安全输入源（HIToolbox 的 `islGetInputSourceIsSecureCandidate` 按 bundle 路径前缀判断），装在 `~/Library/Input Methods` 的那份在密码框等安全输入期间会在输入法菜单里变灰。所以输入法装在系统目录，而那个目录归 root、权限 755：以登录用户身份运行的输入法和设置应用都换不了自己。每次更新都弹管理员密码不可接受，这个程序就是为此存在的唯一特权部件。
//!
//! It is deliberately small and trusts nothing a caller hands it. A caller - any local user - drops a bundle into the inbox by renaming it into place. launchd starts this program while the inbox is not empty (`QueueDirectories`). For each submission it:
//!
//! 1. copies the bundle into a directory only root can reach, so nothing below can change after it is checked;
//! 2. reads the copy's bundle identifier, which must be one of the product's own (`shared/contracts/editions.json`);
//! 3. finds the installed bundle with that identifier in `/Library/Input Methods`. It never installs anything that is not already there: the first install and the move from a per-user install go through an administrator's authorization, not through here;
//! 4. requires the copy to satisfy the code requirement derived from the installed bundle: same identifier, same Developer ID team. The team comes from what an administrator installed, not from this source file and not from the submission;
//! 5. requires a strictly greater build number, so an old, validly signed build cannot be put back;
//! 6. swaps the copy and the installed bundle atomically and deletes the old one.
//!
//! Whatever happens, the submission is removed from the inbox: launchd would otherwise start this program again every few seconds for as long as it stayed there.

#![cfg(target_os = "macos")]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// The launchd label, and the name of everything this program owns on disk.
pub const LABEL: &str = "app.msime.system-updater";

const EDITIONS_JSON: &str = include_str!("../../../shared/contracts/editions.json");

/// 一个输入法 bundle 现在是 217 MB（两个重排模型和语音运行库占大头）。上限留出几倍余量，同时让任何本地用户都没法借这个 root 进程把系统盘写满。
const MAX_BUNDLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_BUNDLE_ENTRIES: u64 = 200_000;
/// 提交方先用点开头的名字把 bundle 拷进收件目录，拷完再改名。还带着点开头名字的条目是正在拷的，放着不动；超过这个时间还没改名的是中断留下的，删掉，否则 launchd 会因为目录非空一直重启本程序。
const ABANDONED_AFTER: Duration = Duration::from_secs(15 * 60);

/// Where this program reads submissions, checks them, and installs them.
#[derive(Debug, Clone)]
pub struct Layout {
    /// World-writable with the sticky bit, like `/tmp`: any user may add a submission, only its owner or root may remove it.
    pub inbox: PathBuf,
    /// Mode 0700, owned by root. A submission is only judged after it has been copied here.
    pub work: PathBuf,
    pub input_methods: PathBuf,
}

impl Layout {
    pub fn system() -> Self {
        let support = Path::new("/Library/Application Support").join(LABEL);
        Self {
            inbox: support.join("inbox"),
            work: support.join("work"),
            input_methods: PathBuf::from("/Library/Input Methods"),
        }
    }
}

/// What this program asks the system about a bundle. A trait so the decisions below can be tested without signed bundles or root.
pub trait Host {
    /// A string value from the bundle's `Contents/Info.plist`.
    fn info_string(&self, bundle: &Path, key: &str) -> Option<String>;
    /// The Team ID in the bundle's code signature.
    fn team_identifier(&self, bundle: &Path) -> Option<String>;
    /// Whether the bundle's signature is intact, sealed resources included, and satisfies `requirement`.
    fn satisfies(&self, bundle: &Path, requirement: &str) -> bool;
}

/// `plutil` and `codesign` by absolute path, with an empty environment.
pub struct SystemHost;

impl SystemHost {
    fn output(program: &str, arguments: &[&OsStr]) -> Option<(bool, Vec<u8>)> {
        let output = Command::new(program)
            .args(arguments)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .ok()?;
        let mut text = output.stdout;
        text.extend_from_slice(&output.stderr);
        Some((output.status.success(), text))
    }
}

impl Host for SystemHost {
    fn info_string(&self, bundle: &Path, key: &str) -> Option<String> {
        let info = bundle.join("Contents/Info.plist");
        let (ok, text) = Self::output(
            "/usr/bin/plutil",
            &[
                OsStr::new("-extract"),
                OsStr::new(key),
                OsStr::new("raw"),
                OsStr::new("-o"),
                OsStr::new("-"),
                info.as_os_str(),
            ],
        )?;
        let value = String::from_utf8(text).ok()?.trim().to_string();
        (ok && !value.is_empty()).then_some(value)
    }

    fn team_identifier(&self, bundle: &Path) -> Option<String> {
        let (ok, text) = Self::output(
            "/usr/bin/codesign",
            &[OsStr::new("-dv"), bundle.as_os_str()],
        )?;
        if !ok {
            return None;
        }
        String::from_utf8_lossy(&text)
            .lines()
            .find_map(|line| line.strip_prefix("TeamIdentifier="))
            .map(|team| team.trim().to_string())
    }

    fn satisfies(&self, bundle: &Path, requirement: &str) -> bool {
        let requirement = format!("-R={requirement}");
        Self::output(
            "/usr/bin/codesign",
            &[
                OsStr::new("--verify"),
                OsStr::new("--deep"),
                OsStr::new("--strict"),
                OsStr::new(&requirement),
                bundle.as_os_str(),
            ],
        )
        .is_some_and(|(ok, _)| ok)
    }
}

/// Why a submission was not installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Not a directory, or it holds something a bundle never holds: a device, a socket, a link that leaves the bundle.
    NotABundle,
    TooLarge,
    /// The bundle identifier is missing or is not one of this product's input methods.
    UnknownIdentifier,
    /// No bundle with this identifier is installed in the system directory.
    NotInstalled,
    /// The installed bundle is not signed with a Developer ID, so there is no team to hold an update to.
    InstalledNotDeveloperId,
    /// The submission's signature is broken, or it is not from the installed bundle's team.
    SignatureMismatch,
    /// The submission is not a newer build than the installed one.
    NotNewer,
    Io,
}

/// What became of one submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Installed { identifier: String, build: u64 },
    Rejected(Rejection),
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Installed { identifier, build } => {
                write!(formatter, "installed {identifier} build {build}")
            }
            Self::Rejected(rejection) => write!(formatter, "rejected: {rejection:?}"),
        }
    }
}

/// Every input method bundle identifier this product ships, one per edition.
fn product_identifiers() -> Vec<String> {
    let table: serde_json::Value =
        serde_json::from_str(EDITIONS_JSON).expect("shared/contracts/editions.json is valid");
    table["editions"]
        .as_array()
        .expect("shared/contracts/editions.json lists editions")
        .iter()
        .filter_map(|edition| edition["platforms"]["macos"]["input_method_bundle_id"].as_str())
        .map(str::to_string)
        .collect()
}

/// Apple 的 Team ID 是 10 位大写字母和数字。它会被拼进代码要求里，所以只接受这个形状。
fn is_team_identifier(team: &str) -> bool {
    team.len() == 10
        && team
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

/// The requirement an update must satisfy: Apple's own shape for "this identifier, signed with this team's Developer ID certificate". `identifier` is one of `product_identifiers` and `team` has passed `is_team_identifier`, so neither can break out of the quotes.
fn update_requirement(identifier: &str, team: &str) -> String {
    format!(
        "anchor apple generic and identifier \"{identifier}\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"{team}\""
    )
}

/// Creates the inbox and the work directory with the modes described on `Layout`, and empties the work directory of anything an interrupted run left.
pub fn prepare(layout: &Layout) -> io::Result<()> {
    for (directory, mode) in [(&layout.inbox, 0o1777), (&layout.work, 0o700)] {
        fs::create_dir_all(directory)?;
        if fs::symlink_metadata(directory)?.file_type().is_symlink() {
            return Err(io::Error::other("a directory of mine is a symbolic link"));
        }
        fs::set_permissions(directory, fs::Permissions::from_mode(mode))?;
    }
    for entry in fs::read_dir(&layout.work)? {
        remove_entry(&entry?.path());
    }
    Ok(())
}

/// Handles every submission in the inbox and leaves it holding only copies still in progress.
pub fn drain(layout: &Layout, host: &dyn Host) -> Vec<(OsString, Outcome)> {
    let mut outcomes = Vec::new();
    let Ok(entries) = fs::read_dir(&layout.inbox) else {
        return outcomes;
    };
    for (index, entry) in entries.flatten().enumerate() {
        let name = entry.file_name();
        let path = entry.path();
        if name.as_encoded_bytes().starts_with(b".") {
            if abandoned(&path) {
                remove_entry(&path);
            }
            continue;
        }
        let copy = layout.work.join(format!("{}-{index}", std::process::id()));
        let outcome = match install(layout, host, &path, &copy) {
            Ok((identifier, build)) => Outcome::Installed { identifier, build },
            Err(rejection) => Outcome::Rejected(rejection),
        };
        remove_entry(&copy);
        remove_entry(&path);
        outcomes.push((name, outcome));
    }
    outcomes
}

fn abandoned(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age > ABANDONED_AFTER)
}

fn remove_entry(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            let _ = fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs::remove_file(path);
        }
        Err(_) => {}
    }
}

fn install(
    layout: &Layout,
    host: &dyn Host,
    submission: &Path,
    copy: &Path,
) -> Result<(String, u64), Rejection> {
    let mut budget = Budget {
        bytes: MAX_BUNDLE_BYTES,
        entries: MAX_BUNDLE_ENTRIES,
    };
    let metadata = fs::symlink_metadata(submission).map_err(|_| Rejection::Io)?;
    if !metadata.is_dir() {
        return Err(Rejection::NotABundle);
    }
    copy_tree(submission, submission, copy, &mut budget)?;

    // 从这里起只看 root 专有目录里的那份拷贝，提交方再改收件目录里的原件也影响不了下面的判断。
    let identifier = host
        .info_string(copy, "CFBundleIdentifier")
        .filter(|identifier| product_identifiers().contains(identifier))
        .ok_or(Rejection::UnknownIdentifier)?;
    let installed = installed_bundle(layout, host, &identifier).ok_or(Rejection::NotInstalled)?;
    let team = host
        .team_identifier(&installed)
        .filter(|team| is_team_identifier(team))
        .ok_or(Rejection::InstalledNotDeveloperId)?;
    let requirement = update_requirement(&identifier, &team);
    if !host.satisfies(&installed, &requirement) {
        return Err(Rejection::InstalledNotDeveloperId);
    }
    if !host.satisfies(copy, &requirement) {
        return Err(Rejection::SignatureMismatch);
    }
    let build = build_number(host, copy).ok_or(Rejection::NotNewer)?;
    // 已装的那份读不出构建号时也不替换：它通过了上面的签名检查，读不出只可能是这里出了问题，而不是它需要修复。修复走管理员授权的那条路。
    let installed_build = build_number(host, &installed).ok_or(Rejection::NotNewer)?;
    if build <= installed_build {
        return Err(Rejection::NotNewer);
    }

    // 先改名到目标旁边，再原子交换：输入法目录里任何时刻都有一份完整的 bundle。交换后 `staged` 这个名字下是旧的那份。
    let staged = layout
        .input_methods
        .join(format!(".{LABEL}.{}.app", std::process::id()));
    remove_entry(&staged);
    fs::rename(copy, &staged).map_err(|_| Rejection::Io)?;
    let swapped = rustix::fs::renameat_with(
        rustix::fs::CWD,
        &staged,
        rustix::fs::CWD,
        &installed,
        rustix::fs::RenameFlags::EXCHANGE,
    );
    remove_entry(&staged);
    swapped.map_err(|_| Rejection::Io)?;
    Ok((identifier, build))
}

fn build_number(host: &dyn Host, bundle: &Path) -> Option<u64> {
    host.info_string(bundle, "CFBundleVersion")?.parse().ok()
}

/// The real directory in the system input method directory whose bundle identifier is `identifier`. Editions are told apart by identifier, not by file name: the file name is a display name and has changed before.
fn installed_bundle(layout: &Layout, host: &dyn Host, identifier: &str) -> Option<PathBuf> {
    fs::read_dir(&layout.input_methods)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(OsStr::new("app")))
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| !name.as_encoded_bytes().starts_with(b"."))
        })
        .filter(|path| fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()))
        .find(|path| host.info_string(path, "CFBundleIdentifier").as_deref() == Some(identifier))
}

struct Budget {
    bytes: u64,
    entries: u64,
}

/// Whether a relative link at `link`, somewhere below `root`, still resolves below `root`. A framework is mostly such links (`Versions/Current -> B`) and the signature seals them as links, so they are recreated rather than followed.
fn link_stays_inside(root: &Path, link: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let Ok(below_root) = link.parent().unwrap_or(root).strip_prefix(root) else {
        return false;
    };
    let mut depth = below_root.components().count();
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Copies a bundle out of the inbox. Directories become 0755 and files 0644 or 0755 whatever the submission says, because the copy is read by every user's input method process; links are recreated only when they stay inside the bundle; anything else is refused. Running as root, everything created here is owned by root.
///
/// 提交方在拷贝过程中把文件换成指向别处的链接也没用：读到的内容只进了 root 专有目录，签名校验不过就整份删掉，谁也读不到它。
fn copy_tree(
    root: &Path,
    source: &Path,
    destination: &Path,
    budget: &mut Budget,
) -> Result<(), Rejection> {
    budget.entries = budget.entries.checked_sub(1).ok_or(Rejection::TooLarge)?;
    let metadata = fs::symlink_metadata(source).map_err(|_| Rejection::Io)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(source).map_err(|_| Rejection::Io)?;
        if !link_stays_inside(root, source, &target) {
            return Err(Rejection::NotABundle);
        }
        return std::os::unix::fs::symlink(&target, destination).map_err(|_| Rejection::Io);
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|_| Rejection::Io)?;
        for entry in fs::read_dir(source).map_err(|_| Rejection::Io)? {
            let entry = entry.map_err(|_| Rejection::Io)?;
            copy_tree(
                root,
                &entry.path(),
                &destination.join(entry.file_name()),
                budget,
            )?;
        }
        return fs::set_permissions(destination, fs::Permissions::from_mode(0o755))
            .map_err(|_| Rejection::Io);
    }
    if !metadata.is_file() {
        return Err(Rejection::NotABundle);
    }
    budget.bytes = budget
        .bytes
        .checked_sub(metadata.len())
        .ok_or(Rejection::TooLarge)?;
    let mut input = fs::File::open(source).map_err(|_| Rejection::Io)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| Rejection::Io)?;
    // 文件在 lstat 之后可能被换成更大的，所以按预算截断着读，而不是相信刚才读到的长度。
    let copied = io::copy(
        &mut io::Read::take(&mut input, metadata.len() + 1),
        &mut output,
    )
    .map_err(|_| Rejection::Io)?;
    if copied > metadata.len() {
        return Err(Rejection::TooLarge);
    }
    let mode = if metadata.permissions().mode() & 0o111 != 0 {
        0o755
    } else {
        0o644
    };
    output
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|_| Rejection::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "app.msime.inputmethod.MetasequoiaIME";
    const TEAM: &str = "ABCDE12345";

    /// Reads a bundle's facts from plain files, in place of `plutil` and `codesign`: `Contents/Info.plist` holds `key=value` lines, `team` holds the signing team, and a file named `tampered` breaks the signature.
    struct FakeHost;

    impl Host for FakeHost {
        fn info_string(&self, bundle: &Path, key: &str) -> Option<String> {
            fs::read_to_string(bundle.join("Contents/Info.plist"))
                .ok()?
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::to_string))
        }

        fn team_identifier(&self, bundle: &Path) -> Option<String> {
            fs::read_to_string(bundle.join("team")).ok()
        }

        fn satisfies(&self, bundle: &Path, requirement: &str) -> bool {
            let identifier = self.info_string(bundle, "CFBundleIdentifier");
            let team = self.team_identifier(bundle);
            !bundle.join("tampered").exists()
                && identifier.is_some_and(|identifier| {
                    requirement.contains(&format!("identifier \"{identifier}\""))
                })
                && team.is_some_and(|team| {
                    requirement.contains(&format!("certificate leaf[subject.OU] = \"{team}\""))
                })
        }
    }

    fn layout(root: &Path) -> Layout {
        let layout = Layout {
            inbox: root.join("inbox"),
            work: root.join("work"),
            input_methods: root.join("Input Methods"),
        };
        fs::create_dir_all(&layout.input_methods).unwrap();
        prepare(&layout).unwrap();
        layout
    }

    fn bundle(path: &Path, identifier: &str, build: u64, team: &str) {
        fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        fs::write(
            path.join("Contents/Info.plist"),
            format!("CFBundleIdentifier={identifier}\nCFBundleVersion={build}\n"),
        )
        .unwrap();
        fs::write(path.join("Contents/MacOS/main"), b"code").unwrap();
        fs::set_permissions(
            path.join("Contents/MacOS/main"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(path.join("team"), team).unwrap();
    }

    fn installed_build(layout: &Layout) -> u64 {
        build_number(&FakeHost, &layout.input_methods.join("水杉输入法.app")).unwrap()
    }

    fn only(outcomes: Vec<(OsString, Outcome)>) -> Outcome {
        assert_eq!(outcomes.len(), 1, "{outcomes:?}");
        outcomes.into_iter().next().unwrap().1
    }

    fn inbox_is_empty(layout: &Layout) -> bool {
        fs::read_dir(&layout.inbox).unwrap().next().is_none()
    }

    #[test]
    fn a_newer_build_from_the_same_team_replaces_the_installed_bundle() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, TEAM);
        bundle(&layout.inbox.join("update.app"), FULL, 11, TEAM);

        assert_eq!(
            only(drain(&layout, &FakeHost)),
            Outcome::Installed {
                identifier: FULL.to_string(),
                build: 11
            }
        );
        assert_eq!(installed_build(&layout), 11);
        assert!(inbox_is_empty(&layout));
        assert!(fs::read_dir(&layout.work).unwrap().next().is_none());
        // 交换用过的临时名字不留在输入法目录里，系统扫这个目录时不该看到第二份。
        let names: Vec<_> = fs::read_dir(&layout.input_methods)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [OsString::from("水杉输入法.app")]);
    }

    #[test]
    fn the_installed_copy_is_readable_by_every_user_whatever_the_submission_says() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        let installed = layout.input_methods.join("水杉输入法.app");
        bundle(&installed, FULL, 10, TEAM);
        let update = layout.inbox.join("update.app");
        bundle(&update, FULL, 11, TEAM);
        fs::set_permissions(
            update.join("Contents/Info.plist"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        drain(&layout, &FakeHost);
        let mode = |path: &str| {
            fs::metadata(installed.join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode("Contents"), 0o755);
        assert_eq!(mode("Contents/Info.plist"), 0o644);
        assert_eq!(mode("Contents/MacOS/main"), 0o755);
    }

    #[test]
    fn the_same_or_an_older_build_is_never_installed() {
        for build in [10, 9] {
            let root = tempfile::tempdir().unwrap();
            let layout = layout(root.path());
            bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, TEAM);
            bundle(&layout.inbox.join("update.app"), FULL, build, TEAM);

            assert_eq!(
                only(drain(&layout, &FakeHost)),
                Outcome::Rejected(Rejection::NotNewer)
            );
            assert_eq!(installed_build(&layout), 10);
            assert!(inbox_is_empty(&layout));
        }
    }

    #[test]
    fn a_build_from_another_team_or_with_a_broken_signature_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, TEAM);
        bundle(&layout.inbox.join("other-team.app"), FULL, 11, "ZZZZZ99999");
        bundle(&layout.inbox.join("tampered.app"), FULL, 11, TEAM);
        fs::write(layout.inbox.join("tampered.app/tampered"), b"").unwrap();

        let outcomes = drain(&layout, &FakeHost);
        assert_eq!(outcomes.len(), 2);
        for (_, outcome) in outcomes {
            assert_eq!(outcome, Outcome::Rejected(Rejection::SignatureMismatch));
        }
        assert_eq!(installed_build(&layout), 10);
        assert!(inbox_is_empty(&layout));
    }

    #[test]
    fn only_this_products_identifiers_are_considered() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        bundle(
            &layout.input_methods.join("Other.app"),
            "com.example.ime",
            1,
            TEAM,
        );
        bundle(&layout.inbox.join("update.app"), "com.example.ime", 2, TEAM);

        assert_eq!(
            only(drain(&layout, &FakeHost)),
            Outcome::Rejected(Rejection::UnknownIdentifier)
        );
        assert_eq!(
            build_number(&FakeHost, &layout.input_methods.join("Other.app")),
            Some(1)
        );
    }

    #[test]
    fn nothing_is_installed_where_no_copy_is_installed_yet() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        bundle(&layout.inbox.join("update.app"), FULL, 11, TEAM);

        assert_eq!(
            only(drain(&layout, &FakeHost)),
            Outcome::Rejected(Rejection::NotInstalled)
        );
        assert!(fs::read_dir(&layout.input_methods)
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn an_installed_copy_without_a_developer_id_team_is_left_alone() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        // 本机开发构建是 ad-hoc 签名，没有 Team ID。
        bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, "");
        bundle(&layout.inbox.join("update.app"), FULL, 11, "");

        assert_eq!(
            only(drain(&layout, &FakeHost)),
            Outcome::Rejected(Rejection::InstalledNotDeveloperId)
        );
        assert_eq!(installed_build(&layout), 10);
    }

    #[test]
    fn each_edition_is_matched_by_identifier_not_by_file_name() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        let wubi = "app.msime.inputmethod.wubi";
        bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, TEAM);
        bundle(&layout.input_methods.join("水杉五笔.app"), wubi, 10, TEAM);
        bundle(&layout.inbox.join("anything.app"), wubi, 12, TEAM);

        assert_eq!(
            only(drain(&layout, &FakeHost)),
            Outcome::Installed {
                identifier: wubi.to_string(),
                build: 12
            }
        );
        assert_eq!(installed_build(&layout), 10);
        assert_eq!(
            build_number(&FakeHost, &layout.input_methods.join("水杉五笔.app")),
            Some(12)
        );
    }

    #[test]
    fn a_submission_that_is_a_link_or_reaches_outside_itself_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        let installed = layout.input_methods.join("水杉输入法.app");
        bundle(&installed, FULL, 10, TEAM);
        let secret = root.path().join("secret");
        fs::write(&secret, b"root only").unwrap();

        // 提交本身是指向别处的链接。
        std::os::unix::fs::symlink(&installed, layout.inbox.join("link.app")).unwrap();
        // bundle 里有一个指到外面的链接。
        let escaping = layout.inbox.join("escaping.app");
        bundle(&escaping, FULL, 11, TEAM);
        std::os::unix::fs::symlink(&secret, escaping.join("Contents/secret")).unwrap();
        // 相对链接向上走出了 bundle。
        let climbing = layout.inbox.join("climbing.app");
        bundle(&climbing, FULL, 11, TEAM);
        std::os::unix::fs::symlink("../../../secret", climbing.join("Contents/secret")).unwrap();

        let outcomes = drain(&layout, &FakeHost);
        assert_eq!(outcomes.len(), 3);
        for (name, outcome) in outcomes {
            assert_eq!(
                outcome,
                Outcome::Rejected(Rejection::NotABundle),
                "{name:?}"
            );
        }
        assert_eq!(installed_build(&layout), 10);
        assert!(inbox_is_empty(&layout));
        assert_eq!(fs::read(&secret).unwrap(), b"root only");
    }

    #[test]
    fn links_that_stay_inside_the_bundle_are_kept_as_links() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        let installed = layout.input_methods.join("水杉输入法.app");
        bundle(&installed, FULL, 10, TEAM);
        let update = layout.inbox.join("update.app");
        bundle(&update, FULL, 11, TEAM);
        let versions = update.join("Contents/Frameworks/Sparkle.framework/Versions");
        fs::create_dir_all(versions.join("B")).unwrap();
        std::os::unix::fs::symlink("B", versions.join("Current")).unwrap();

        assert!(matches!(
            only(drain(&layout, &FakeHost)),
            Outcome::Installed { .. }
        ));
        let link = installed.join("Contents/Frameworks/Sparkle.framework/Versions/Current");
        assert_eq!(fs::read_link(link).unwrap(), Path::new("B"));
    }

    #[test]
    fn a_copy_still_in_progress_is_left_alone_and_an_abandoned_one_is_removed() {
        let root = tempfile::tempdir().unwrap();
        let layout = layout(root.path());
        bundle(&layout.input_methods.join("水杉输入法.app"), FULL, 10, TEAM);
        let copying = layout.inbox.join(".copying.app");
        bundle(&copying, FULL, 11, TEAM);

        assert!(drain(&layout, &FakeHost).is_empty());
        assert!(copying.exists());
        assert_eq!(installed_build(&layout), 10);

        let long_ago = SystemTime::now() - ABANDONED_AFTER - Duration::from_secs(60);
        fs::File::open(&copying)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        assert!(drain(&layout, &FakeHost).is_empty());
        assert!(inbox_is_empty(&layout));
    }

    #[test]
    fn the_requirement_names_the_identifier_and_the_installed_team() {
        assert_eq!(
            update_requirement(FULL, TEAM),
            "anchor apple generic and identifier \"app.msime.inputmethod.MetasequoiaIME\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"ABCDE12345\""
        );
        assert!(is_team_identifier("LXCL4Z68GU"));
        for team in ["", "lxcl4z68gu", "LXCL4Z68G", "LXCL4Z68GU\"", "LXCL4Z68GU1"] {
            assert!(!is_team_identifier(team), "{team:?}");
        }
    }

    #[test]
    fn every_edition_has_an_identifier_this_program_serves() {
        let identifiers = product_identifiers();
        assert!(identifiers.contains(&FULL.to_string()));
        assert!(identifiers.len() >= 6, "{identifiers:?}");
        for identifier in identifiers {
            assert!(
                identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-'),
                "{identifier} would not be safe inside a code requirement"
            );
        }
    }
}
