use super::*;
use crate::skin::theme::GlobalTheme;

/// 1×1 的合成 PNG。
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

fn manifest(id: &str, name: &str) -> String {
    format!(
        "schema_version = 1\nid = \"{id}\"\nname = \"{name}\"\nversion = \"1.0\"\nbase = \"system\"\npreview = \"preview.png\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\nmin_width_dip = 0\n[candidate.light]\naccent = \"#0067C0\"\nsurface = \"#FFFFFF\"\n"
    )
}

fn draft(id: &str, name: &str) -> CandidateSkinDraft {
    CandidateSkinDraft {
        package_id: id.into(),
        manifest: manifest(id, name),
        files: BTreeMap::from([("preview.png".into(), PNG.into())]),
    }
}

#[test]
fn a_template_manifest_resolves_the_look_palette_without_images() {
    let template = CandidateSkinDraft {
        package_id: "template".into(),
        manifest: "schema_version = 1\nid = \"template\"\nname = \"模板\"\nversion = \"1\"\nbase = \"wechat\"\n[supports]\nlayouts = [\"horizontal\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\n".into(),
        files: BTreeMap::new(),
    };
    let summary = check(&template).unwrap();
    // 外观画在 `system` 上，没写的颜色按外观补齐。
    assert_eq!(summary.base, GlobalTheme::System);
    assert_eq!(summary.candidate.light.accent.as_deref(), Some("#07C160"));
    assert_eq!(summary.candidate.dark.surface.as_deref(), Some("#151515"));
    assert_eq!(summary.corner_radius_dip, Some(5.0));
}

#[test]
fn check_reports_why_the_manifest_does_not_load() {
    let mut broken = draft("broken", "坏皮肤");
    broken.manifest = broken
        .manifest
        .replace("min_width_dip = 0", "min_width_dip = 5000");
    assert_eq!(
        check(&broken).unwrap_err(),
        CandidateSkinFailure {
            code: "candidate_skin_package",
            detail: Some("invalid min_width_dip".into()),
        }
    );
    let mut reserved = draft("night", "夜");
    reserved.manifest = manifest("night", "夜");
    assert_eq!(check(&reserved).unwrap_err().code, "candidate_skin_id");
    let mut corrupt = draft("corrupt", "坏图");
    corrupt
        .files
        .insert("preview.png".into(), "not base64!".into());
    assert_eq!(
        check(&corrupt).unwrap_err().code,
        "candidate_skin_image_invalid"
    );
    let mut large = draft("large", "大");
    large
        .manifest
        .push_str(&format!("# {}\n", "x".repeat(MAX_MANIFEST_BYTES)));
    assert_eq!(check(&large).unwrap_err().code, "candidate_skin_too_large");
}

#[test]
fn create_installs_through_the_shared_installer() {
    let root = tempfile::tempdir().unwrap();
    let created = create(root.path(), &draft("my-skin", "我的皮肤"), false).unwrap();
    assert_eq!(
        created,
        CreatedCandidateSkin {
            id: "my-skin".into(),
            replaced: false,
        }
    );
    let scanned = catalog::scan(root.path());
    assert!(scanned.issues.is_empty());
    assert_eq!(scanned.packages.len(), 1);
    assert_eq!(scanned.packages[0].name, "我的皮肤");
    assert_eq!(scanned.packages[0].preview.as_deref(), Some("preview.png"));
}

#[test]
fn an_existing_skin_is_replaced_only_when_asked() {
    let root = tempfile::tempdir().unwrap();
    create(root.path(), &draft("my-skin", "第一版"), false).unwrap();
    assert_eq!(
        create(root.path(), &draft("my-skin", "第二版"), false).unwrap_err(),
        CandidateSkinFailure::code("candidate_skin_exists")
    );
    assert_eq!(catalog::scan(root.path()).packages[0].name, "第一版");
    let replaced = create(root.path(), &draft("my-skin", "第二版"), true).unwrap();
    assert!(replaced.replaced);
    assert_eq!(catalog::scan(root.path()).packages[0].name, "第二版");
    // 没有同名皮肤时 `replace` 不算替换。
    assert!(
        !create(root.path(), &draft("other", "另一款"), true)
            .unwrap()
            .replaced
    );
}

#[test]
fn create_explains_a_refused_package() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        create(root.path(), &draft("Upper", "大写"), false).unwrap_err(),
        CandidateSkinFailure::code("candidate_skin_id")
    );
    let mut broken = draft("broken", "坏皮肤");
    broken.manifest = broken
        .manifest
        .replace("base = \"system\"", "base = \"custom\"");
    assert_eq!(
        create(root.path(), &broken, false).unwrap_err(),
        CandidateSkinFailure {
            code: "candidate_skin_package",
            detail: Some("base must be system or a built-in theme".into()),
        }
    );
    // 清单能读，但没有预览图：分享规则不收，`detail` 为空。
    let mut previewless = draft("previewless", "无预览");
    previewless.manifest = previewless
        .manifest
        .replace("preview = \"preview.png\"\n", "")
        .replace(
            "[candidate_window]\nmin_width_dip = 0\n",
            "[candidate_window]\nmin_width_dip = 0\n[candidate_window.background]\nimage = \"preview.png\"\n",
        );
    assert_eq!(
        create(root.path(), &previewless, false).unwrap_err(),
        CandidateSkinFailure::code("candidate_skin_package")
    );
    // 没有任何图片时安装器先按图片数量拒绝。
    let mut imageless = draft("imageless", "无图");
    imageless.files.clear();
    assert_eq!(
        create(root.path(), &imageless, false).unwrap_err(),
        CandidateSkinFailure::code("candidate_skin_file_path")
    );
    assert!(catalog::scan(root.path()).packages.is_empty());
}
