//! 网页包带的辅助码表必须正好是引擎认得的那几套：`scripts/build-web-engine.sh` 打进 npm 包的表、SDK 导出的 `HELPCODES` 和 `msime_engine::assets::HELPCODES` 一一对应，顺序也相同。引擎增减一套辅助码而网页包没跟上，这里就会失败。
#![cfg(not(target_family = "wasm"))]

use std::path::{Path, PathBuf};

use msime_engine::assets::HELPCODES;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_web_package_ships_every_engine_helpcode_table() {
    let root = repo_root();
    let script = read(&root.join("scripts/build-web-engine.sh"));
    // 打包脚本里写成 `方案:文件`，顺序与引擎相同。
    let mut at = 0;
    for (schema, file) in HELPCODES {
        let entry = format!("{schema}:{file}");
        let found = script[at..].find(&entry).unwrap_or_else(|| {
            panic!(
                "scripts/build-web-engine.sh does not package {entry} after the tables before it"
            )
        });
        at += found + entry.len();
        assert!(
            root.join("resources").join(file).is_file(),
            "resources/{file} is missing"
        );
    }

    let sdk = read(&root.join("packages/web-engine/src/index.js"));
    let names: Vec<String> = HELPCODES
        .iter()
        .map(|(schema, _)| format!("\"{schema}\""))
        .collect();
    let declaration = format!(
        "export const HELPCODES = Object.freeze([{}]);",
        names.join(", ")
    );
    assert!(
        sdk.contains(&declaration),
        "packages/web-engine/src/index.js must declare {declaration}"
    );
    let types = read(&root.join("packages/web-engine/src/index.d.ts"));
    // 格式化工具会把成员多的联合类型拆成每行一个 `| "名字"`，所以比较前去掉空白和开头那个 `|`。
    let union = names.join("|");
    let declared = types
        .split_once("export type MsimeHelpcode =")
        .and_then(|(_, rest)| rest.split_once(';'))
        .map(|(body, _)| body.split_whitespace().collect::<String>())
        .expect("packages/web-engine/src/index.d.ts declares MsimeHelpcode");
    assert_eq!(
        declared.trim_start_matches('|'),
        union,
        "packages/web-engine/src/index.d.ts must declare MsimeHelpcode as {union}"
    );
}
