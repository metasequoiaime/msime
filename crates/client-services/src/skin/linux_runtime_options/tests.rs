use super::*;
use crate::skin::catalog::scan;
use serde_json::json;

fn write_skin(root: &Path, id: &str, name: &str, surface: &str) {
    let package = root.join(id);
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("skin.toml"),
        format!("schema_version = 1\nid = '{id}'\nname = '{name}'\nversion = '1.0'\nbase = 'paper'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '{surface}'\nselected = '#ff69b4'\ntext = '#301020'\nnumber = '#605060'\naccent = '#ff1493'\nhover = '#ffe4ec'\n[candidate.dark]\nsurface = '#301020'\ntext = '#ffe4e1'\nborder = '#ff000080'\nnumber = '#c0a0b0'\naccent = '#ff69b4'\nhover = '#402030'\n"),
    )
    .unwrap();
}

/// 按安装器的方式整包换掉：新目录写好后 rename 到原位置，id 不变。
fn replace_skin(root: &Path, id: &str, name: &str, surface: &str) {
    let staging = root.join(".staging");
    write_skin(&staging, id, name, surface);
    std::fs::remove_dir_all(root.join(id)).unwrap();
    std::fs::rename(staging.join(id), root.join(id)).unwrap();
    std::fs::remove_dir_all(staging).unwrap();
}

fn surface(document: &Value, id: &str) -> Value {
    document["candidate_skin_catalog"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["id"] == id)
        .map(|package| package["candidate"]["light"]["surface"].clone())
        .unwrap_or(Value::Null)
}

#[test]
fn a_republish_writes_only_when_what_the_hosts_draw_changed() {
    let directory = tempfile::tempdir().unwrap();
    let skins = directory.path().join("skins");
    write_skin(&skins, "sakura", "樱花", "#fff0f5");
    let mut document = json!({"api_version": 1, "resources": "/resources", "preferences": {}});

    let first = republish_skin_catalog(&mut document, &skins, &scan(&skins))
        .unwrap()
        .expect("a new skin is published");
    assert_eq!(serde_json::from_slice::<Value>(&first).unwrap(), document);
    assert_eq!(document["resources"], "/resources");
    assert_eq!(surface(&document, "sakura"), "#FFF0F5");

    // 什么都没变时不写，监视文件的宿主不会白白重载。
    assert_eq!(
        republish_skin_catalog(&mut document, &skins, &scan(&skins)).unwrap(),
        None
    );

    // 同名替换成另一种配色：条目变了，宿主比较的文档跟着变。
    replace_skin(&skins, "sakura", "樱花", "#e0ffe0");
    let replaced = republish_skin_catalog(&mut document, &skins, &scan(&skins))
        .unwrap()
        .expect("a replaced skin with new colours is republished");
    assert_eq!(surface(&document, "sakura"), "#E0FFE0");
    assert_eq!(
        serde_json::from_slice::<Value>(&replaced).unwrap(),
        document
    );

    // 再整包换一次、内容与上一次相同：目录换了，宿主画的条目没变，不写。
    replace_skin(&skins, "sakura", "樱花", "#e0ffe0");
    assert_eq!(
        republish_skin_catalog(&mut document, &skins, &scan(&skins)).unwrap(),
        None
    );

    // 删掉之后发布更短的目录。
    std::fs::remove_dir_all(skins.join("sakura")).unwrap();
    republish_skin_catalog(&mut document, &skins, &scan(&skins))
        .unwrap()
        .expect("a deleted skin is unpublished");
    assert_eq!(document["candidate_skin_catalog"], json!({"packages": []}));
}

#[test]
fn the_skin_catalog_stays_within_what_the_hosts_read() {
    let directory = tempfile::tempdir().unwrap();
    let skins = directory.path().join("skins");
    for index in 0..40 {
        write_skin(
            &skins,
            &format!("skin{index:02}"),
            &format!("皮肤 {index:02}"),
            "#fff0f5",
        );
    }
    let catalog = scan(&skins);
    // 按名字排在最后的包：超出包数上限、裁剪时最先被丢，除非两处都保护选中的皮肤。深色槽位同理。
    let mut document = json!({"api_version": 1, "preferences": {"custom_theme": {"candidate_skin": "skin39", "candidate_skin_dark": "skin38"}}});
    let bytes = runtime_options_with_skin_catalog(&mut document, &skins, &catalog).unwrap();
    assert!(
        bytes.len() <= LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET,
        "{}",
        bytes.len()
    );
    let packages = document["candidate_skin_catalog"]["packages"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        !packages.is_empty() && packages.len() < 32,
        "{}",
        packages.len()
    );
    assert!(packages.iter().any(|package| package["id"] == "skin39"));
    assert!(packages.iter().any(|package| package["id"] == "skin38"));
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), document);

    // 已经放不下皮肤目录的文档不会被目录撑得更大。
    let mut crowded =
        json!({"preferences": {}, "padding": "x".repeat(LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET)});
    runtime_options_with_skin_catalog(&mut crowded, &skins, &catalog).unwrap();
    assert!(crowded.get("candidate_skin_catalog").is_none());

    // 去掉目录后仍超出宿主上限的文档被拒绝，调用方因此不写文件。
    let mut oversized =
        json!({"preferences": {}, "padding": "x".repeat(LINUX_RUNTIME_OPTIONS_LIMIT)});
    assert_eq!(
        republish_skin_catalog(&mut oversized, &skins, &catalog),
        Err(TooLarge)
    );
}
