//! 手写识别由引擎负责，macOS 这边只负责找到模型文件。
//!
//! 发布包不再内置手写模型，第一次打开手写面板时下载到 `<state_root>/resource-packs/handwriting/`，查找时优先用这份已下载的模型（三个桌面平台共用的 `downloaded_handwriting_model`）；从内置模型的旧版本升级上来、还没下载时，退回 app 里的 `Contents/Resources/handwriting/`（[`bundled_model`]）。
use std::path::{Path, PathBuf};

pub(crate) fn bundled_model(executable: &Path) -> Option<PathBuf> {
    if !executable.is_absolute() {
        return None;
    }
    let directory = executable.parent()?;
    if directory.file_name()? != "MacOS" {
        return None;
    }
    let contents = directory.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let model = contents.join("Resources/handwriting/handwriting-zh_CN.model");
    model.is_file().then_some(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_input_runtime::{HandwritingPoint, HandwritingQuery};

    /// The Engine's ordered-stroke 中 fixture, mapped through `place`.
    fn zhong_strokes(place: impl Fn((f32, f32)) -> (f32, f32)) -> Vec<Vec<(f32, f32)>> {
        vec![
            vec![(35., 40.), (35., 105.)],
            vec![(35., 40.), (125., 40.), (125., 105.)],
            vec![(35., 105.), (125., 105.)],
            vec![(80., 15.), (80., 140.)],
        ]
        .into_iter()
        .map(|stroke| stroke.into_iter().map(&place).collect())
        .collect()
    }

    fn query(strokes: Vec<Vec<(f32, f32)>>) -> HandwritingQuery {
        HandwritingQuery {
            language: "zh-CN".into(),
            strokes: strokes
                .into_iter()
                .map(|stroke| {
                    stroke
                        .into_iter()
                        .map(|(x, y)| HandwritingPoint { x, y })
                        .collect()
                })
                .collect(),
        }
    }

    /// The pinned model (resources/handwriting-model.lock.json): `MSIME_HANDWRITING_MODEL`, else where `scripts/fetch_handwriting_model.py` puts it. It is a 26.8 MB download, so the recognition cases are skipped, with the reason printed, when it has not been fetched.
    fn engine_model() -> Option<PathBuf> {
        let path = std::env::var_os("MSIME_HANDWRITING_MODEL")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../target/handwriting-model/handwriting-zh_CN.model")
            });
        if path.is_file() {
            return Some(path);
        }
        eprintln!(
            "skipped: no handwriting model at {}; run scripts/fetch_handwriting_model.py or set MSIME_HANDWRITING_MODEL",
            path.display()
        );
        None
    }

    #[test]
    fn relocated_bundle_finds_the_packaged_model() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("Synthetic.app/Contents/MacOS/synthetic");
        assert!(bundled_model(&executable).is_none());
        let resource = root
            .path()
            .join("Synthetic.app/Contents/Resources/handwriting");
        std::fs::create_dir_all(&resource).unwrap();
        let model = resource.join("handwriting-zh_CN.model");
        std::fs::write(&model, b"placeholder").unwrap();
        assert_eq!(bundled_model(&executable), Some(model));
        assert!(bundled_model(Path::new("Synthetic.app/Contents/MacOS/synthetic")).is_none());
        assert!(bundled_model(&root.path().join("synthetic")).is_none());
    }

    #[test]
    fn the_packaged_model_recognizes_a_single_character() {
        let Some(resolved) = engine_model() else {
            return;
        };
        // Synthetic 中, the same ordered-stroke fixture used by the engine.
        let query = query(zhong_strokes(|(x, y)| (x, y)));
        let candidates =
            msime_host_api::handwriting_local_candidates(resolved.to_str().unwrap(), &query)
                .unwrap();
        assert!(candidates.iter().any(|candidate| candidate == "中"));
        assert!(candidates.len() <= 12);
    }

    #[test]
    fn a_line_of_two_characters_is_recognized_as_one_multi_character_candidate() {
        // 中 written twice side by side, each about 150 px tall in the 420 px panel canvas.
        let scale = 1.2;
        let mut strokes =
            zhong_strokes(|(x, y)| (20. + (x - 35.) * scale, 130. + (y - 15.) * scale));
        strokes.extend(zhong_strokes(|(x, y)| {
            (220. + (x - 35.) * scale, 130. + (y - 15.) * scale)
        }));
        let Some(model) = engine_model() else {
            return;
        };
        let candidates =
            msime_host_api::handwriting_local_candidates(model.to_str().unwrap(), &query(strokes))
                .unwrap();
        assert_eq!(candidates.first().map(String::as_str), Some("中中"));
        assert!(candidates.len() <= 12);
    }

    #[test]
    fn macos_package_downloads_the_model_on_demand() {
        let configuration: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.macos.conf.json")).unwrap();
        let resources = configuration["bundle"]["resources"].as_object().unwrap();
        assert!(resources
            .values()
            .any(|path| path == "handwriting/Zinnia-LICENSE.txt"));
        assert_eq!(configuration["bundle"]["active"], true);
        // 发布包不再带手写模型：首次打开手写面板时由 App 下载到 resource-packs/handwriting。打包脚本要在编译前用同一个安装器确认资源包可下载，并断言包里没有模型。
        let package = include_str!("../../../../../../platforms/macos/package-release.sh");
        assert!(package.contains("install_resource_pack"));
        assert!(
            package.contains(r#"test ! -e "$resources_dir/handwriting/handwriting-zh_CN.model""#)
        );
        assert!(!package.contains("fetch_handwriting_model.py"));
    }
}
