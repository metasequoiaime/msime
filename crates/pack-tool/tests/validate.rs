//! `msime-pack validate` against real pack folders: the report lines, the exit status, and that the rules are client-core's.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use msime_pack_tool::{run, EXIT_INVALID, EXIT_OK, EXIT_USAGE};

fn template() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-template")
}

/// Run the command line and return the exit status, standard output and standard error.
fn invoke(args: &[&Path]) -> (i32, String, String) {
    let args: Vec<OsString> = args.iter().map(|arg| arg.as_os_str().to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &mut out, &mut err);
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn manifest(directory: &Path, body: &str) {
    fs::create_dir_all(directory).unwrap();
    fs::write(
        directory.join("plugin.toml"),
        format!("schema_version = 1\nname = \"Test\"\nversion = \"0.1.0\"\nlicense = \"CC0-1.0\"\n{body}"),
    )
    .unwrap();
}

#[test]
fn the_documented_template_is_a_valid_sound_pack() {
    let (status, out, err) = invoke(&[Path::new("validate"), &template()]);
    assert_eq!(
        (status, out.as_str(), err.as_str()),
        (EXIT_OK, "ok example-keys sound 1.0.0\n", "")
    );
}

#[test]
fn every_pack_gets_one_line_in_order_and_any_failure_fails_the_run() {
    let packs = tempfile::tempdir().unwrap();
    let neon = packs.path().join("neon");
    manifest(
        &neon,
        "kind = \"effect\"\nid = \"neon\"\n[effect]\nstyle = \"sparks\"\ncolors = [\"#00FFCC\", \"#ff00aa\"]\nduration_ms = 400\nparticles = 24\n",
    );
    let loud = packs.path().join("loud");
    manifest(
        &loud,
        "kind = \"effect\"\nid = \"loud\"\n[effect]\nstyle = \"sparks\"\nintensity = 101\n",
    );
    // A sound pack naming an Ogg sample: music may stream Ogg, but a key sample must be WAV.
    let ogg = packs.path().join("ogg-keys");
    manifest(
        &ogg,
        "kind = \"sound\"\nid = \"ogg-keys\"\n[sounds]\ndefault = \"key.ogg\"\n",
    );
    fs::write(ogg.join("key.ogg"), b"OggS\0\x02\0\0\0\0\0\0\0\0").unwrap();
    let missing = packs.path().join("missing");

    let (status, out, err) = invoke(&[
        Path::new("validate"),
        &neon,
        &loud,
        &template(),
        &ogg,
        &missing,
    ]);
    assert_eq!(status, EXIT_INVALID);
    assert!(err.is_empty());
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 5, "{out}");
    assert_eq!(lines[0], "ok neon effect 0.1.0");
    assert_eq!(
        lines[1],
        format!(
            "error {}: plugin_invalid: effect.intensity 必须是 0 到 100 之间的整数",
            loud.display()
        )
    );
    assert_eq!(lines[2], "ok example-keys sound 1.0.0");
    assert_eq!(
        lines[3],
        format!(
            "error {}: plugin_invalid: key.ogg 不是 .wav 文件名，音效包只接受 WAV 音频",
            ogg.display()
        )
    );
    assert_eq!(
        lines[4],
        format!(
            "error {}: plugin_unsupported_source: 只接受插件文件夹或 .zip 文件",
            missing.display()
        )
    );
}

#[test]
fn a_built_in_id_is_refused_as_import_refuses_it() {
    let packs = tempfile::tempdir().unwrap();
    let pack = packs.path().join("default");
    fs::create_dir_all(&pack).unwrap();
    for name in ["plugin.toml", "key.wav", "space.wav"] {
        fs::copy(template().join(name), pack.join(name)).unwrap();
    }
    let manifest = fs::read_to_string(pack.join("plugin.toml")).unwrap();
    fs::write(
        pack.join("plugin.toml"),
        manifest.replace("id = \"example-keys\"", "id = \"default\""),
    )
    .unwrap();
    let (status, out, _) = invoke(&[Path::new("validate"), &pack]);
    assert_eq!(status, EXIT_INVALID);
    assert_eq!(
        out,
        format!(
            "error {}: plugin_reserved: 这个 id 属于内置插件，不能导入\n",
            pack.display()
        )
    );
}

#[test]
fn a_command_line_it_does_not_understand_is_a_usage_error() {
    for args in [
        &[][..],
        &[Path::new("validate")][..],
        &[Path::new("check"), &template()][..],
    ] {
        let (status, out, err) = invoke(args);
        assert_eq!(status, EXIT_USAGE, "{args:?}");
        assert!(out.is_empty());
        assert!(err.starts_with("usage: msime-pack validate"));
    }
    let (status, out, _) = invoke(&[Path::new("--help")]);
    assert_eq!(status, EXIT_OK);
    assert!(out.starts_with("usage: msime-pack validate"));
}

#[test]
fn the_binary_reports_and_exits_the_same_way() {
    let output = Command::new(env!("CARGO_BIN_EXE_msime-pack"))
        .arg("validate")
        .arg(template())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(EXIT_OK));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "ok example-keys sound 1.0.0\n"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_msime-pack"))
        .arg("validate")
        .arg(template().join("plugin.toml"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(EXIT_INVALID));
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("error "));
}

/// client-core 与社区后端共用的 fixture 包：`valid/` 下的每个都报 ok，`invalid/` 下的每个都报 plugin_invalid，退出码随之而定。
#[test]
fn the_shared_fixture_packs_validate_as_their_folder_says() {
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../client-core/tests/fixtures/plugin-packs");
    for (group, expected_status) in [("valid", EXIT_OK), ("invalid", EXIT_INVALID)] {
        let mut packs: Vec<PathBuf> = fs::read_dir(fixtures.join(group))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        packs.sort();
        assert!(!packs.is_empty(), "{group}");
        for pack in packs {
            let (status, out, err) = invoke(&[Path::new("validate"), &pack]);
            assert_eq!(status, expected_status, "{}: {out}{err}", pack.display());
            let prefix = if expected_status == EXIT_OK {
                "ok ".to_owned()
            } else {
                format!("error {}: plugin_invalid: ", pack.display())
            };
            assert!(out.starts_with(&prefix), "{}: {out}", pack.display());
        }
    }
}
