//! 脚本插件（只在桌面）：清单的版本与权限规则、`[script]` 表、模块校验、内容哈希、导入，以及 `script-grants.json` 的读写。
//!
//! 模块用手写的 wasm 二进制：client-core 不依赖 wat，测试也不提交 `.wasm`。

use super::script_grants::{self, ScriptGrantError, ScriptGrants};
use super::script_pack::{self, ScriptPack};
use super::*;
use msime_script_runtime::{limits, Capability, CapabilitySet, Hook, HookSet};
use tempfile::tempdir;

const I32: u8 = 0x7f;
const I64: u8 = 0x7e;

/// 一个 import：名字、参数类型、结果类型。
type Import = (&'static str, &'static [u8], &'static [u8]);

/// `msime` 白名单里几个 import 的签名。
const COMMIT_SCALAR: Import = ("commit_scalar", &[I32], &[I64]);
const COMMIT_TEXT: Import = ("commit_text", &[I32, I32, I32], &[I32]);
const REWRITE_COMMIT: Import = ("rewrite_commit", &[I32, I32], &[I32]);
const COMPOSITION_SCALAR: Import = ("composition_scalar", &[I32], &[I64]);

fn leb(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn name(text: &str, out: &mut Vec<u8>) {
    leb(text.len(), out);
    out.extend_from_slice(text.as_bytes());
}

fn section(id: u8, content: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    leb(content.len(), out);
    out.extend_from_slice(content);
}

/// 最小的 ABI v1 模块：一页内存导出为 `memory`，`msime_abi` 返回 1，`msime_on_event` 返回 0，按 `imports` 从 `msime` 导入函数。
fn module(imports: &[Import]) -> Vec<u8> {
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    let signatures: Vec<(&[u8], &[u8])> = imports
        .iter()
        .map(|(_, params, results)| (*params, *results))
        .chain([(&[][..], &[I32][..]), (&[I32][..], &[I32][..])])
        .collect();
    let mut types = Vec::new();
    leb(signatures.len(), &mut types);
    for (params, results) in &signatures {
        types.push(0x60);
        leb(params.len(), &mut types);
        types.extend_from_slice(params);
        leb(results.len(), &mut types);
        types.extend_from_slice(results);
    }
    section(1, &types, &mut out);
    if !imports.is_empty() {
        let mut content = Vec::new();
        leb(imports.len(), &mut content);
        for (index, (import, _, _)) in imports.iter().enumerate() {
            name("msime", &mut content);
            name(import, &mut content);
            content.push(0x00);
            leb(index, &mut content);
        }
        section(2, &content, &mut out);
    }
    let mut functions = vec![2];
    leb(imports.len(), &mut functions);
    leb(imports.len() + 1, &mut functions);
    section(3, &functions, &mut out);
    section(5, &[1, 0x00, 1], &mut out);
    let mut exports = vec![3];
    name("memory", &mut exports);
    exports.extend_from_slice(&[0x02, 0]);
    name("msime_abi", &mut exports);
    exports.push(0x00);
    leb(imports.len(), &mut exports);
    name("msime_on_event", &mut exports);
    exports.push(0x00);
    leb(imports.len() + 1, &mut exports);
    section(7, &exports, &mut out);
    section(10, &[2, 4, 0, 0x41, 1, 0x0b, 4, 0, 0x41, 0, 0x0b], &mut out);
    out
}

/// 在模块末尾加一个自定义段，把整个文件补到恰好 `total` 字节。
fn padded(mut wasm: Vec<u8>, total: usize) -> Vec<u8> {
    // 段长用 3 字节的 LEB128 写（足够 1 MiB），名字 `pad` 占 4 字节。
    let payload = total - wasm.len() - 1 - 3 - 4;
    let length = 4 + payload;
    wasm.push(0);
    wasm.extend_from_slice(&[
        (length & 0x7f) as u8 | 0x80,
        ((length >> 7) & 0x7f) as u8 | 0x80,
        (length >> 14) as u8,
    ]);
    name("pad", &mut wasm);
    wasm.resize(total, 0);
    wasm
}

fn manifest(id: &str, permissions: &str, script: &str) -> String {
    format!("schema_version = 2\nkind = 'script'\nid = '{id}'\nname = '盘古之白'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = [{permissions}]\n[script]\n{script}")
}

const SCRIPT: &str = "abi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\n";

/// 只读上屏文本的包：申请 `read_commit`，导入 `commit_scalar`。
fn reader(id: &str) -> (String, Vec<u8>) {
    (
        manifest(id, "'read_commit'", SCRIPT),
        module(&[COMMIT_SCALAR]),
    )
}

/// 在 `parent/<id>` 写一个脚本包，返回包目录。
fn write_pack(parent: &Path, id: &str, manifest: &str, wasm: &[u8]) -> PathBuf {
    let directory = parent.join(id);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(MANIFEST_FILE), manifest).unwrap();
    fs::write(directory.join("main.wasm"), wasm).unwrap();
    directory
}

fn refusal(directory: &Path) -> String {
    match validate(directory) {
        Err(PluginError::Invalid(reason)) => reason,
        other => panic!("not refused as invalid: {other:?}"),
    }
}

fn script_content(summary: &PluginSummary) -> &ScriptPack {
    match &summary.content {
        PluginContent::Script(pack) => pack,
        other => panic!("not a script pack: {other:?}"),
    }
}

/// 导入 `parent/<id>` 的包，返回它的内容哈希。
fn import_pack(parent: &Path, root: &Path, id: &str, manifest: &str, wasm: &[u8]) -> String {
    let source = write_pack(parent, id, manifest, wasm);
    let summary = import(&source, root).unwrap();
    script_content(&summary).content_hash.clone()
}

#[test]
fn schema_version_and_permissions_are_checked_by_kind() {
    let files = tempdir().unwrap();
    let (script, wasm) = reader("pangu");
    let command_table = "schema_version = 1\nkind = 'command_table'\nid = 'sig'\nname = '签名'\nversion = '1'\nlicense = 'CC0-1.0'\n[[commands]]\ntrigger = 'sig'\ntitle = '签名'\ntemplate = '测试'\n";

    // script 只认 2，其他类型只认 1；1 和 2 以外的版本对哪种类型都先报版本。
    let directory = write_pack(files.path(), "pangu", &script, &wasm);
    assert_eq!(validate(&directory).unwrap().kind(), PluginKind::Script);
    for (from, to, expected) in [
        (
            "schema_version = 2",
            "schema_version = 1",
            "script 插件必须写 schema_version = 2",
        ),
        (
            "schema_version = 2",
            "schema_version = 3",
            "不支持这个 schema_version",
        ),
        ("schema_version = 2\n", "", "不支持这个 schema_version"),
        ("kind = 'script'", "kind = 'scripts'", "不是已知的插件类型"),
    ] {
        fs::write(directory.join(MANIFEST_FILE), script.replacen(from, to, 1)).unwrap();
        assert!(refusal(&directory).contains(expected), "{to}");
    }

    let directory = files.path().join("sig");
    fs::create_dir_all(&directory).unwrap();
    for (version, accepted) in [(1, true), (2, false), (3, false)] {
        fs::write(
            directory.join(MANIFEST_FILE),
            command_table.replacen(
                "schema_version = 1",
                &format!("schema_version = {version}"),
                1,
            ),
        )
        .unwrap();
        if accepted {
            assert_eq!(
                validate(&directory).unwrap().kind(),
                PluginKind::CommandTable
            );
        } else {
            assert!(refusal(&directory).contains("不支持这个 schema_version"));
        }
    }
    // 数据类插件照旧不能申请任何权限，白名单里的也不行。
    fs::write(
        directory.join(MANIFEST_FILE),
        format!("permissions = ['read_commit']\n{command_table}"),
    )
    .unwrap();
    assert!(refusal(&directory).contains("插件不能申请权限，permissions 必须为空"));
}

#[test]
fn script_permissions_are_a_whitelist_with_dependencies_and_one_exclusion() {
    let files = tempdir().unwrap();
    let cases: &[(&str, &[Import], &str)] = &[
        ("'network'", &[], "permissions 里有未知的权限 network"),
        (
            "'read_commit', 'read_commit'",
            &[COMMIT_SCALAR],
            "read_commit 重复了",
        ),
        ("1", &[], "每一项都必须是字符串"),
        (
            "'rewrite_commit'",
            &[REWRITE_COMMIT],
            "申请 rewrite_commit 必须同时申请 read_commit",
        ),
        (
            "'rerank_candidates'",
            &[],
            "申请 rerank_candidates 必须同时申请 read_candidates",
        ),
        (
            "'read_commit', 'rewrite_commit', 'read_composition'",
            &[COMMIT_SCALAR, REWRITE_COMMIT, COMPOSITION_SCALAR],
            "rewrite_commit 不能和 read_composition 同时申请",
        ),
    ];
    for (permissions, imports, expected) in cases {
        let directory = write_pack(
            files.path(),
            "pangu",
            &manifest("pangu", permissions, SCRIPT),
            &module(imports),
        );
        let reason = refusal(&directory);
        assert!(reason.contains(expected), "{permissions}: {reason}");
    }
    let directory = write_pack(
        files.path(),
        "pangu",
        &manifest("pangu", "", SCRIPT).replace("permissions = []", "permissions = 'all'"),
        &module(&[]),
    );
    assert!(refusal(&directory).contains("permissions 必须是数组"));

    // 读和改写上屏一起申请是 pangu 示例的组合，可以。
    let directory = write_pack(
        files.path(),
        "pangu",
        &manifest("pangu", "'read_commit', 'rewrite_commit'", SCRIPT),
        &module(&[COMMIT_TEXT, REWRITE_COMMIT]),
    );
    let summary = validate(&directory).unwrap();
    assert_eq!(
        script_content(&summary).permissions,
        CapabilitySet::EMPTY
            .with(Capability::ReadCommit)
            .with(Capability::RewriteCommit)
    );
    // 不写 permissions 等于不申请：只用不需要能力的 import 也是合法的脚本。
    let directory = write_pack(
        files.path(),
        "pangu",
        &manifest("pangu", "", SCRIPT).replace("permissions = []\n", ""),
        &module(&[]),
    );
    assert!(script_content(&validate(&directory).unwrap())
        .permissions
        .is_empty());
}

#[test]
fn the_script_table_and_its_module_are_checked_without_running_anything() {
    let files = tempdir().unwrap();
    let (script, wasm) = reader("pangu");
    let cases: &[(&str, &str, &str)] = &[
        ("[script]\n", "[scripts]\n", "未知的键 scripts"),
        (
            "abi = 1\n",
            "abi = 1\nentry = 'main'\n",
            "script 里有未知的键 entry",
        ),
        ("abi = 1", "abi = 2", "script.abi 只能是 1"),
        ("abi = 1\n", "", "script.abi 只能是 1"),
        (
            "module = 'main.wasm'",
            "module = '../main.wasm'",
            "文件名 ../main.wasm 无效",
        ),
        (
            "module = 'main.wasm'",
            "module = 'missing.wasm'",
            "缺少数据文件 missing.wasm",
        ),
        (
            "module = 'main.wasm'",
            "module = 'main.txt'",
            "main.txt 的扩展名必须是 .wasm",
        ),
        ("hooks = ['commit']", "hooks = []", "至少要有一个钩子"),
        (
            "hooks = ['commit']",
            "hooks = 'commit'",
            "script.hooks 必须是数组",
        ),
        (
            "hooks = ['commit']",
            "hooks = ['key_down']",
            "未知的钩子 key_down",
        ),
        (
            "hooks = ['commit']",
            "hooks = ['commit', 'commit']",
            "commit 重复了",
        ),
        (
            "hooks = ['commit']\n",
            "hooks = ['commit']\nmemory_pages = 0\n",
            "memory_pages 必须是 1 到 64",
        ),
        (
            "hooks = ['commit']\n",
            "hooks = ['commit']\nmemory_pages = 65\n",
            "memory_pages 必须是 1 到 64",
        ),
        (
            "hooks = ['commit']\n",
            "hooks = ['commit']\nmemory_pages = '16'\n",
            "memory_pages 必须是 1 到 64",
        ),
    ];
    for (from, to, expected) in cases {
        let changed = script.replacen(from, to, 1);
        assert_ne!(changed, script, "{from}");
        let directory = write_pack(files.path(), "pangu", &changed, &wasm);
        let reason = refusal(&directory);
        assert!(reason.contains(expected), "{to}: {reason}");
    }
    let without_table = format!("{}\n", script.split("[script]").next().unwrap().trim_end());
    let directory = write_pack(files.path(), "pangu", &without_table, &wasm);
    assert!(refusal(&directory).contains("脚本插件缺少 script 表"));

    // 模块本身：不是 wasm、导入了没申请的能力、申请了却没用到、超过 256 KiB。
    let modules: &[(&str, Vec<u8>, &str)] = &[
        (
            "'read_commit'",
            b"(module)".to_vec(),
            "main.wasm 不是有效的脚本模块",
        ),
        (
            "",
            module(&[COMMIT_SCALAR]),
            "needs permission `read_commit`",
        ),
        ("'read_commit'", module(&[]), "no import that uses it"),
        (
            "'read_commit'",
            padded(
                module(&[COMMIT_SCALAR]),
                script_pack::MAX_MODULE_BYTES as usize + 1,
            ),
            "main.wasm 为空或太大",
        ),
        ("'read_commit'", Vec::new(), "main.wasm 为空或太大"),
    ];
    for (permissions, wasm, expected) in modules {
        let directory = write_pack(
            files.path(),
            "pangu",
            &manifest("pangu", permissions, SCRIPT),
            wasm,
        );
        let reason = refusal(&directory);
        assert!(reason.contains(expected), "{expected}: {reason}");
    }
    // 恰好 256 KiB 的模块可以。
    let directory = write_pack(
        files.path(),
        "pangu",
        &script,
        &padded(
            module(&[COMMIT_SCALAR]),
            script_pack::MAX_MODULE_BYTES as usize,
        ),
    );
    validate(&directory).unwrap();
}

#[test]
fn a_script_pack_is_summarized_with_its_declaration_and_content_hash() {
    let files = tempdir().unwrap();
    let script = manifest(
        "pangu",
        "'read_commit', 'rewrite_commit'",
        "abi = 1\nmodule = 'main.wasm'\nhooks = ['before_commit', 'commit']\nmemory_pages = 4\n",
    );
    let wasm = module(&[COMMIT_TEXT, REWRITE_COMMIT]);
    let directory = write_pack(files.path(), "pangu", &script, &wasm);
    fs::write(directory.join("README.md"), "说明").unwrap();
    let summary = validate(&directory).unwrap();
    let pack = script_content(&summary);
    assert_eq!(pack.module, "main.wasm");
    assert_eq!(
        pack.hooks,
        HookSet::EMPTY.with(Hook::BeforeCommit).with(Hook::Commit)
    );
    assert_eq!(pack.memory_pages, 4);
    assert_eq!(
        pack.content_hash,
        script_pack::content_hash(script.as_bytes(), &wasm)
    );
    assert_eq!(pack.declaration().memory_pages, 4);
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["kind"], "script");
    assert_eq!(
        json["hooks"],
        serde_json::json!(["before_commit", "commit"])
    );
    assert_eq!(
        json["permissions"],
        serde_json::json!(["read_commit", "rewrite_commit"])
    );
    assert_eq!(json["content_hash"], pack.content_hash.as_str());
    assert!(json.get("module_bytes").is_none());

    // 默认 16 页。
    let (script, wasm) = reader("pangu");
    let directory = write_pack(files.path(), "pangu", &script, &wasm);
    let summary = validate(&directory).unwrap();
    assert_eq!(
        script_content(&summary).memory_pages,
        limits::DEFAULT_MEMORY_PAGES
    );
}

#[test]
fn the_content_hash_covers_the_manifest_and_the_module_but_not_notices() {
    let files = tempdir().unwrap();
    let (script, wasm) = reader("pangu");
    let directory = write_pack(files.path(), "pangu", &script, &wasm);
    let hash = |directory: &Path| {
        script_content(&validate(directory).unwrap())
            .content_hash
            .clone()
    };
    let original = hash(&directory);
    assert_eq!(original.len(), 64);
    fs::write(directory.join("NOTICE.txt"), "另一份说明").unwrap();
    assert_eq!(hash(&directory), original);
    fs::write(
        directory.join(MANIFEST_FILE),
        script.replace("version = '1.0.0'", "version = '1.0.1'"),
    )
    .unwrap();
    assert_ne!(hash(&directory), original);
    fs::write(directory.join(MANIFEST_FILE), &script).unwrap();
    assert_eq!(hash(&directory), original);
    fs::write(
        directory.join("main.wasm"),
        padded(wasm.clone(), wasm.len() + 16),
    )
    .unwrap();
    assert_ne!(hash(&directory), original);
    // 两段之间有长度分隔：字节挪到另一段里，哈希也不同。
    assert_ne!(
        script_pack::content_hash(b"ab", b"c"),
        script_pack::content_hash(b"a", b"bc")
    );
}

/// `msime-pack run` 读包的入口：规则同 `validate`，交出的字节就是哈希的那一份；不是脚本包、或者包本身不合规时报和 `validate` 同样的错误。
#[test]
fn load_script_source_hands_over_what_validate_accepted() {
    let files = tempdir().unwrap();
    let (script, wasm) = reader("pangu");
    let directory = write_pack(files.path(), "pangu", &script, &wasm);
    let loaded = load_script_source(&directory).unwrap();
    assert_eq!(loaded.id, "pangu");
    assert_eq!(loaded.wasm, wasm);
    assert_eq!(
        loaded.pack.content_hash,
        script_pack::content_hash(script.as_bytes(), &wasm)
    );
    assert_eq!(loaded.pack.module_bytes, None);

    let phrases = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plugin-packs/valid/phrase_table-basic");
    assert!(validate(&phrases).is_ok());
    assert!(matches!(
        load_script_source(&phrases),
        Err(PluginError::Invalid(reason)) if reason.contains("只有脚本插件")
    ));

    fs::write(directory.join("main.wasm"), b"not wasm").unwrap();
    assert_eq!(
        load_script_source(&directory).unwrap_err().to_string(),
        validate(&directory).unwrap_err().to_string()
    );
    assert!(matches!(
        load_script_source(&files.path().join("missing")),
        Err(PluginError::UnsupportedSource)
    ));
}

#[test]
fn load_hands_over_the_module_it_hashed_and_scan_lists_scripts_that_pages_leave_out() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    assert!(root.join("script/pangu/main.wasm").is_file());

    let loaded = script_pack::load(&root, "pangu").unwrap();
    assert_eq!(loaded.id, "pangu");
    assert_eq!(loaded.wasm, wasm);
    assert_eq!(loaded.pack.content_hash, hash);
    assert_eq!(loaded.pack.module_bytes, None);
    assert!(script_pack::load(&root, "missing").is_err());
    assert!(script_pack::load(&root, "../pangu").is_err());

    // 目录里列得出脚本包和坏掉的脚本文件夹，设置页的目录命令把它们都去掉。
    fs::create_dir_all(root.join("script/broken")).unwrap();
    let catalog = scan(&root, None);
    assert_eq!(catalog.packages.len(), 1);
    assert_eq!(catalog.packages[0].kind(), PluginKind::Script);
    assert_eq!(catalog.issues.len(), 1);
    assert_eq!(catalog.issues[0].kind, PluginKind::Script);
    let listed = catalog.without_scripts();
    assert!(listed.packages.is_empty());
    assert!(listed.issues.is_empty());

    // 脚本插件不能发布到社区。
    assert!(!community::PUBLISHABLE_KINDS.contains(&PluginKind::Script));
    assert!(community::pack(&root, PluginKind::Script, "pangu").is_err());
}

#[test]
fn the_settings_import_refuses_script_packs_in_release_builds_before_installing_anything() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let source = write_pack(files.path(), "pangu", &script, &wasm);

    // release 构建里设置页的导入走的就是这条路：和移动端遇到 `script` 一样报未知类型，不留包目录，也不写授权。
    match super::import::import_inner(&source, &root, false) {
        Err(PluginError::Invalid(reason)) => assert_eq!(reason, UNKNOWN_KIND),
        other => panic!("not refused as an unknown kind: {other:?}"),
    }
    assert!(!root.join("script/pangu").exists());
    assert!(!root.join(script_grants::GRANTS_FILE).exists());
    assert!(script_grants::read(&root).plugins.is_empty());

    // 没有授权页的设置宿主只在开发构建里接受脚本包。
    assert_eq!(
        super::import::import_without_script_page(&source, &root).is_ok(),
        cfg!(debug_assertions)
    );
    assert_eq!(root.join("script/pangu").is_dir(), cfg!(debug_assertions));

    // 有授权页的设置宿主（桌面外壳的设置页、Windows 的原生设置应用）在三个桌面平台的开发构建和 release 构建里都接受脚本包。
    let other = tempdir().unwrap();
    let other_root = other.path().join("plugins");
    assert!(import_from_settings(&source, &other_root).is_ok());
    assert!(other_root.join("script/pangu").is_dir());
}

fn read_commit() -> CapabilitySet {
    CapabilitySet::EMPTY.with(Capability::ReadCommit)
}

#[test]
fn grants_are_bound_to_the_content_hash() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    assert_eq!(script_grants::read(&root), ScriptGrants::default());

    let grants = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    // 每次写入 revision 都变大，不小于当前时间的微秒数。
    let mut revision = grants.revision;
    assert!(revision > 1 << 50 && revision < script_grants::MAX_REVISION);
    let mut advanced = |grants: &ScriptGrants| {
        assert!(
            grants.revision > revision,
            "{} <= {revision}",
            grants.revision
        );
        revision = grants.revision;
    };
    assert_eq!(script_grants::read(&root), grants);
    let enabled: Vec<_> = grants.enabled().map(|grant| grant.id.as_str()).collect();
    assert_eq!(enabled, ["pangu"]);
    assert!(script_grants::document_path(&root).is_file());

    // 授权页展示之后包被换过：哈希对不上就不写。
    assert!(matches!(
        script_grants::set(&root, "pangu", &"0".repeat(64), true, read_commit()),
        Err(ScriptGrantError::Changed)
    ));
    assert!(matches!(
        script_grants::set(&root, "other", &hash, true, read_commit()),
        Err(ScriptGrantError::Missing(_))
    ));

    // 原样重新导入：授权保留，revision 变大。
    assert_eq!(
        import_pack(files.path(), &root, "pangu", &script, &wasm),
        hash
    );
    let grants = script_grants::read(&root);
    advanced(&grants);
    assert!(grants.get("pangu").unwrap().enabled);
    assert_eq!(grants.get("pangu").unwrap().granted, read_commit());

    // 导入了不同的内容：停用并清空授予项，保留旧哈希。
    let updated = script.replace("version = '1.0.0'", "version = '1.1.0'");
    let new_hash = import_pack(files.path(), &root, "pangu", &updated, &wasm);
    assert_ne!(new_hash, hash);
    let grants = script_grants::read(&root);
    advanced(&grants);
    let entry = grants.get("pangu").unwrap();
    assert!(!entry.enabled);
    assert!(entry.granted.is_empty());
    assert_eq!(entry.hash, hash);
    assert_eq!(grants.enabled().count(), 0);

    // 用新哈希重新授权。
    let grants = script_grants::set(&root, "pangu", &new_hash, true, read_commit()).unwrap();
    advanced(&grants);
    assert_eq!(grants.get("pangu").unwrap().hash, new_hash);
    // 撤销：条目还在，停用。
    let grants =
        script_grants::set(&root, "pangu", &new_hash, false, CapabilitySet::EMPTY).unwrap();
    advanced(&grants);
    assert!(!grants.get("pangu").unwrap().enabled);
}

#[test]
fn removing_a_script_pack_drops_its_grant_so_a_reimport_starts_ungranted() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    let granted = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();

    remove_named(&root, "script", "pangu").unwrap();
    assert!(!root.join("script/pangu").exists());
    let grants = script_grants::read(&root);
    assert!(grants.revision > granted.revision);
    assert!(grants.plugins.is_empty());

    import_pack(files.path(), &root, "pangu", &script, &wasm);
    assert!(script_grants::read(&root).plugins.is_empty());

    // 包目录被手工删掉之后再导入同样的内容，留下的条目也不会复活。
    script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    fs::remove_dir_all(root.join("script/pangu")).unwrap();
    import_pack(files.path(), &root, "pangu", &script, &wasm);
    let grants = script_grants::read(&root);
    assert!(!grants.get("pangu").unwrap().enabled);
}

#[test]
fn granted_capabilities_are_a_subset_of_the_manifest_and_follow_the_rules() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let hash = import_pack(
        files.path(),
        &root,
        "pangu",
        &manifest("pangu", "'read_commit', 'rewrite_commit'", SCRIPT),
        &module(&[COMMIT_TEXT, REWRITE_COMMIT]),
    );
    let refused =
        |granted: CapabilitySet| match script_grants::set(&root, "pangu", &hash, true, granted) {
            Err(ScriptGrantError::Invalid(reason)) => reason,
            other => panic!("{granted:?}: {other:?}"),
        };
    assert!(refused(read_commit().with(Capability::ReadComposition))
        .contains("清单没有申请 read_composition"));
    assert!(
        refused(CapabilitySet::EMPTY.with(Capability::RewriteCommit))
            .contains("申请 rewrite_commit 必须同时申请 read_commit")
    );
    // 少授几项可以：实际生效的是交集。
    script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    script_grants::set(
        &root,
        "pangu",
        &hash,
        true,
        read_commit().with(Capability::RewriteCommit),
    )
    .unwrap();
    assert!(matches!(
        script_grants::set(&root, "../pangu", &hash, true, read_commit()),
        Err(ScriptGrantError::Invalid(_))
    ));
    // 设置页传来的能力名：认识的换成集合，不认识的整个拒绝，而不是悄悄丢掉一项。
    assert_eq!(
        script_grants::capabilities(&["rewrite_commit".into(), "read_commit".into()]).unwrap(),
        read_commit().with(Capability::RewriteCommit)
    );
    assert!(matches!(
        script_grants::capabilities(&["read_commit".into(), "teleport".into()]),
        Err(ScriptGrantError::Invalid(reason)) if reason == "未知的能力 teleport"
    ));
}

#[test]
fn unreadable_or_unknown_grant_files_enable_nothing() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    let document = script_grants::document_path(&root);
    let entry = |id: &str, hash: &str, granted: &str| {
        format!("{{\"id\":\"{id}\",\"hash\":\"{hash}\",\"enabled\":true,\"granted\":[{granted}]}}")
    };

    // 不认识的格式版本：什么都不启用，也不覆盖它。
    fs::write(
        &document,
        format!(
            "{{\"format_version\":2,\"revision\":7,\"plugins\":[{}]}}",
            entry("pangu", &hash, "\"read_commit\"")
        ),
    )
    .unwrap();
    assert_eq!(script_grants::read(&root), ScriptGrants::default());
    assert!(matches!(
        script_grants::set(&root, "pangu", &hash, true, read_commit()),
        Err(ScriptGrantError::Format)
    ));
    // 不是合法 JSON：什么都不启用，写入时整份重写。
    fs::write(&document, b"{\"format_version\":1,").unwrap();
    assert_eq!(script_grants::read(&root), ScriptGrants::default());
    script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    assert_eq!(script_grants::read(&root).enabled().count(), 1);

    // 未知字段照常读；不合规的条目只跳过它自己；同一个 id 只认第一条；不认识的能力名丢掉。
    fs::write(
        &document,
        format!(
            "{{\"format_version\":1,\"revision\":9,\"future\":true,\"plugins\":[{},{},{},{},{}]}}",
            entry("bad id", &hash, "\"read_commit\""),
            entry("short", "abc", "\"read_commit\""),
            entry(
                "exclusive",
                &hash,
                "\"read_commit\",\"rewrite_commit\",\"read_composition\""
            ),
            entry("pangu", &hash, "\"read_commit\",\"teleport\"").replace('}', ",\"note\":\"x\"}"),
            entry("pangu", &hash, "").replace("true", "false"),
        ),
    )
    .unwrap();
    let grants = script_grants::read(&root);
    assert_eq!(grants.revision, 9);
    assert_eq!(grants.plugins.len(), 1);
    let pangu = grants.get("pangu").unwrap();
    assert!(pangu.enabled);
    assert_eq!(pangu.granted, read_commit());

    // 符号链接不跟随。
    #[cfg(unix)]
    {
        let elsewhere = files.path().join("grants.json");
        fs::rename(&document, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &document).unwrap();
        assert_eq!(script_grants::read(&root), ScriptGrants::default());
    }
}

#[test]
fn enabling_is_bounded_by_the_script_count_and_the_memory_budget() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let mut hashes = Vec::new();
    for index in 0..=limits::MAX_ENABLED_SCRIPTS {
        let id = format!("small{index}");
        let (script, wasm) = reader(&id);
        hashes.push((
            id.clone(),
            import_pack(files.path(), &root, &id, &script, &wasm),
        ));
    }
    let (last, rest) = hashes.split_last().unwrap();
    for (id, hash) in rest {
        script_grants::set(&root, id, hash, true, read_commit()).unwrap();
    }
    assert!(matches!(
        script_grants::set(&root, &last.0, &last.1, true, read_commit()),
        Err(ScriptGrantError::Limit)
    ));
    // 停用一个就腾出位置；停用不受上限约束。
    script_grants::set(&root, &rest[0].0, &rest[0].1, false, read_commit()).unwrap();
    script_grants::set(&root, &last.0, &last.1, true, read_commit()).unwrap();
    let grants = script_grants::read(&root);
    assert_eq!(grants.enabled().count(), limits::MAX_ENABLED_SCRIPTS);
    // 新条目排在最后执行。
    assert_eq!(grants.plugins.last().unwrap().id, last.0);

    // 256 KiB、64 页的模块：线性内存 4 MiB，编译结果按上界估算 32 MiB，一个就占掉总账的一半多，第二个放不下。
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let big = padded(
        module(&[COMMIT_SCALAR]),
        script_pack::MAX_MODULE_BYTES as usize,
    );
    let fitting = limits::PROCESS_MEMORY_BYTES
        / limits::script_memory_bytes(big.len(), limits::MAX_MEMORY_PAGES);
    assert!(fitting < limits::MAX_ENABLED_SCRIPTS);
    let mut hashes = Vec::new();
    for index in 0..=fitting {
        let id = format!("big{index}");
        let script = manifest(
            &id,
            "'read_commit'",
            &format!("{SCRIPT}memory_pages = {}\n", limits::MAX_MEMORY_PAGES),
        );
        hashes.push((
            id.clone(),
            import_pack(files.path(), &root, &id, &script, &big),
        ));
    }
    let (last, rest) = hashes.split_last().unwrap();
    for (id, hash) in rest {
        script_grants::set(&root, id, hash, true, read_commit()).unwrap();
    }
    assert!(matches!(
        script_grants::set(&root, &last.0, &last.1, true, read_commit()),
        Err(ScriptGrantError::Memory)
    ));
    script_grants::set(&root, &last.0, &last.1, false, read_commit()).unwrap();
}

#[test]
fn grant_failures_carry_their_own_codes() {
    for (error, code) in [
        (
            ScriptGrantError::Missing("x".into()),
            "plugin_script_missing",
        ),
        (ScriptGrantError::Changed, "plugin_script_changed"),
        (
            ScriptGrantError::Invalid("x".into()),
            "plugin_script_grant_invalid",
        ),
        (ScriptGrantError::Limit, "plugin_script_limit"),
        (ScriptGrantError::Memory, "plugin_script_memory"),
        (ScriptGrantError::Format, "plugin_script_grants_format"),
        (ScriptGrantError::Storage, "plugin_storage"),
    ] {
        assert_eq!(PluginFailure::from(error).code, code);
    }
}

#[test]
fn a_leftover_mac_field_from_earlier_builds_still_loads_and_runs() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    let (other_script, other_wasm) = reader("other");
    let other_hash = import_pack(files.path(), &root, "other", &other_script, &other_wasm);
    // 早期开发构建在 macOS 上给条目写过 `mac`，有的条目没有它。授权不再签名，这个字段当作未知字段忽略，没有它也不拒绝任何条目。
    let document = script_grants::document_path(&root);
    fs::write(
        &document,
        serde_json::to_vec(&serde_json::json!({
            "format_version": 1,
            "revision": 7,
            "plugins": [
                {"id": "pangu", "hash": hash, "enabled": true, "granted": ["read_commit"], "mac": "0".repeat(64)},
                {"id": "other", "hash": other_hash, "enabled": true, "granted": ["read_commit"]},
            ],
        }))
        .unwrap(),
    )
    .unwrap();
    let grants = script_grants::read(&root);
    assert_eq!(grants.revision, 7);
    assert_eq!(
        grants
            .enabled()
            .map(|grant| (grant.id.as_str(), grant.hash.as_str(), grant.granted))
            .collect::<Vec<_>>(),
        [
            ("pangu", hash.as_str(), read_commit()),
            ("other", other_hash.as_str(), read_commit())
        ]
    );
    // 下一次写入照常，写出的文件里不再有 `mac`。
    let grants = script_grants::reorder(&root, &["other".into()]).unwrap();
    assert!(grants.revision > 7);
    assert_eq!(script_grants::read(&root), grants);
    assert!(!String::from_utf8(fs::read(&document).unwrap())
        .unwrap()
        .contains("\"mac\""));
}

#[test]
fn reordering_changes_the_execution_order() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let mut ids = Vec::new();
    for id in ["first", "second", "third"] {
        let (script, wasm) = reader(id);
        let hash = import_pack(files.path(), &root, id, &script, &wasm);
        script_grants::set(&root, id, &hash, true, read_commit()).unwrap();
        ids.push(id.to_owned());
    }
    let order = |grants: &ScriptGrants| -> Vec<String> {
        grants.enabled().map(|grant| grant.id.clone()).collect()
    };
    let before = script_grants::read(&root);
    assert_eq!(order(&before), ["first", "second", "third"]);

    let grants = script_grants::reorder(&root, &["third".into(), "first".into()]).unwrap();
    assert!(grants.revision > before.revision);
    // 列出的排在最前，没列出的按原来的相对顺序排在后面，照常运行。
    assert_eq!(order(&grants), ["third", "first", "second"]);
    assert_eq!(script_grants::read(&root), grants);

    for (bad, reason) in [
        (vec!["missing".to_owned()], "没有 missing 的授权"),
        (vec!["first".to_owned(), "first".to_owned()], "first 重复了"),
    ] {
        match script_grants::reorder(&root, &bad) {
            Err(ScriptGrantError::Invalid(message)) => assert_eq!(message, reason),
            other => panic!("{bad:?}: {other:?}"),
        }
    }
    assert_eq!(script_grants::read(&root), grants);
}

#[test]
fn a_new_content_hash_voids_the_grant_even_when_the_file_still_names_the_old_one() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    // 包被换了内容（不经导入，直接改文件）：条目仍写着旧哈希，读出来和已安装的包对不上（宿主据此不运行它，见 host-api 的 `a_grant_whose_hash_no_longer_matches_the_pack_never_runs`）。授权页拿旧哈希写入被拒绝，拿现在的哈希重新授权才生效。
    fs::write(
        root.join("script/pangu/plugin.toml"),
        script.replace("version = '1.0.0'", "version = '1.0.1'"),
    )
    .unwrap();
    let installed = script_pack::load(&root, "pangu").unwrap();
    assert_ne!(installed.pack.content_hash, hash);
    let grants = script_grants::read(&root);
    let entry = grants.get("pangu").unwrap();
    assert_eq!(entry.hash, hash);
    assert_ne!(entry.hash, installed.pack.content_hash);
    assert!(matches!(
        script_grants::set(&root, "pangu", &hash, true, read_commit()),
        Err(ScriptGrantError::Changed)
    ));
    let grants = script_grants::set(
        &root,
        "pangu",
        &installed.pack.content_hash,
        true,
        read_commit(),
    )
    .unwrap();
    assert_eq!(
        grants.get("pangu").unwrap().hash,
        installed.pack.content_hash
    );
}

#[test]
fn a_forged_revision_neither_freezes_nor_rewinds_the_writer() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let (script, wasm) = reader("pangu");
    let hash = import_pack(files.path(), &root, "pangu", &script, &wasm);
    let document = script_grants::document_path(&root);

    // 一份伪造的文件把 revision 写成 `u64::MAX`：之后的写入不接着它加，每次写入的 revision 仍然各不相同、一次比一次大。
    fs::write(
        &document,
        format!(
            "{{\"format_version\":1,\"revision\":{},\"plugins\":[]}}",
            u64::MAX
        ),
    )
    .unwrap();
    // 输入法进程不能先记住这个极大 revision：设置页随后写出的正常值比它小，
    // 熔断状态就永远不会因授权变化而恢复。
    assert_eq!(
        script_grants::read(&root),
        script_grants::ScriptGrants::default()
    );
    let first = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    let second = script_grants::set(&root, "pangu", &hash, false, read_commit()).unwrap();
    assert!(first.revision < script_grants::MAX_REVISION);
    assert!(second.revision > first.revision);
    assert!(second.revision < script_grants::MAX_REVISION);

    // 把 revision 改小（或者放回一份旧文件）之后，下一次写入的 revision 仍然比以前写出的都大，宿主的熔断照样随它复位。
    let written = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    fs::write(
        &document,
        "{\"format_version\":1,\"revision\":1,\"plugins\":[]}",
    )
    .unwrap();
    let after = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    assert!(after.revision > written.revision);

    // 合法范围内的最后一个 revision 触发回绕：写入方推进纪元，之后
    // 继续在新纪元内单调增加，宿主可据此忽略旧文件的回放。
    fs::write(
        &document,
        format!(
            "{{\"format_version\":1,\"revision\":{},\"plugins\":[]}}",
            script_grants::MAX_REVISION - 1
        ),
    )
    .unwrap();
    let wrapped = script_grants::set(&root, "pangu", &hash, true, read_commit()).unwrap();
    assert_eq!(wrapped.revision_epoch, 1);
    assert!(wrapped.revision < script_grants::MAX_REVISION - 1);
    let next = script_grants::set(&root, "pangu", &hash, false, read_commit()).unwrap();
    assert_eq!(next.revision_epoch, 1);
    assert!(next.revision > wrapped.revision);
}

#[test]
fn admission_counts_every_place_the_input_method_fills() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let mut packs = Vec::new();
    for index in 0..=limits::MAX_ENABLED_SCRIPTS {
        let id = format!("small{index}");
        let (script, wasm) = reader(&id);
        let hash = import_pack(files.path(), &root, &id, &script, &wasm);
        packs.push((id, hash, script));
    }
    let (last, rest) = packs.split_last().unwrap();
    for (id, hash, _) in rest {
        script_grants::set(&root, id, hash, true, read_commit()).unwrap();
    }
    // 一个包被直接改了内容：它的条目哈希对不上、不会运行，但输入法取前 8 个启用条目时它占着一个位置，新启用的插件会排在第 9 位、永远不运行，所以照样拒绝。
    let (stale, _, script) = &rest[0];
    fs::write(
        root.join("script").join(stale).join(MANIFEST_FILE),
        script.replace("version = '1.0.0'", "version = '1.0.1'"),
    )
    .unwrap();
    assert!(matches!(
        script_grants::set(&root, &last.0, &last.1, true, read_commit()),
        Err(ScriptGrantError::Limit)
    ));
    // 一个包目录被直接删掉：写入时顺带去掉它的条目，腾出的位置给新启用的插件。
    let (gone, _, _) = &rest[1];
    fs::remove_dir_all(root.join("script").join(gone)).unwrap();
    let grants = script_grants::set(&root, &last.0, &last.1, true, read_commit()).unwrap();
    assert!(grants.get(gone).is_none());
    assert!(grants.get(stale).is_some());
    assert!(grants.enabled().any(|grant| grant.id == last.0));
    assert_eq!(grants.enabled().count(), limits::MAX_ENABLED_SCRIPTS);
}

#[test]
fn the_health_file_lists_only_the_four_disabled_categories() {
    let user_data = tempdir().unwrap();
    assert!(script_grants::health(Some(user_data.path())).is_empty());
    assert!(script_grants::health(Some(Path::new("relative"))).is_empty());
    assert!(script_grants::health(None).is_empty());
    let file = user_data.path().join(script_grants::HEALTH_FILE);
    std::fs::write(
        &file,
        r#"{"format_version":1,"plugins":[{"id":"a","hash":"h1","disabled":"failed"},{"id":"b","hash":"h2","disabled":"unreachable"},{"id":"c"},{"id":"d","hash":"h4","disabled":"slow"}]}"#,
    )
    .unwrap();
    assert_eq!(
        script_grants::health(Some(user_data.path())),
        [
            script_grants::ScriptHealthView {
                id: "a".into(),
                hash: "h1".into(),
                disabled: "failed".into(),
            },
            script_grants::ScriptHealthView {
                id: "d".into(),
                hash: "h4".into(),
                disabled: "slow".into(),
            },
        ]
    );
    // 不认识的格式或读不懂的文件当作没有停用记录。
    std::fs::write(
        &file,
        r#"{"format_version":2,"plugins":[{"id":"a","hash":"h1","disabled":"failed"}]}"#,
    )
    .unwrap();
    assert!(script_grants::health(Some(user_data.path())).is_empty());
    std::fs::write(&file, "not json").unwrap();
    assert!(script_grants::health(Some(user_data.path())).is_empty());
}

#[test]
fn the_grants_view_keeps_the_order_and_the_capability_names() {
    let capabilities = |names: &[&str]| {
        script_grants::capabilities(
            &names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    };
    let grant = |id: &str, enabled: bool, granted| script_grants::ScriptGrant {
        id: id.into(),
        hash: id.repeat(64 / id.len()),
        enabled,
        granted,
    };
    let rewriting = capabilities(&["rewrite_commit", "read_commit"]);
    let view = script_grants::view(
        ScriptGrants {
            revision_epoch: 0,
            revision: 4,
            plugins: vec![
                grant("b", true, rewriting),
                grant("a", false, capabilities(&[])),
            ],
        },
        None,
    );
    assert_eq!(view.revision, 4);
    // 文件里的顺序就是执行顺序，原样交给页面；能力名按 `Capability::ALL` 的顺序。
    assert_eq!(
        view.plugins,
        [
            script_grants::ScriptGrantView {
                id: "b".into(),
                hash: "b".repeat(64),
                enabled: true,
                granted: vec!["read_commit", "rewrite_commit"],
            },
            script_grants::ScriptGrantView {
                id: "a".into(),
                hash: "a".repeat(64),
                enabled: false,
                granted: Vec::new(),
            },
        ]
    );
    assert!(view.health.is_empty());
}
