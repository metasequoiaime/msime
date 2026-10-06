//! External candidate-window skins as an agent may see and make them.
//!
//! A skin is a folder under `skins` in the state directory: a `skin.toml` manifest and the PNG or JPEG images it references. The desktop app keeps that folder in step with the signed-in user's cloud library, so a skin written here is uploaded as a private package the next time the app starts or its community page opens. The server itself never signs in: the account session belongs to the desktop app, whose refresh token is rotated on every use, so a second process holding it would sign the user out.
//!
//! A skin is written by the same rule the community gallery installs a download by (`candidate_community::install`): the manifest verbatim plus exactly the images it references, a preview among them, and no stylesheet. Every skin made here can therefore be synced and shared without further changes.

use msime_client_core::skin::candidate_community::{self, CandidateSkinPackage};
use msime_client_core::skin::{candidate_sync, catalog};
use msime_client_core::uuid::Uuid;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub const SKIN_DIRECTORY: &str = "skins";

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinList {
    pub skins: Vec<SkinView>,
    /// Folders under the skin directory that are not a usable skin, with the reason the input method gives.
    pub issues: Vec<SkinIssueView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinView {
    /// The folder name and the manifest id.
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The candidate layouts the skin supports: horizontal, vertical.
    pub layouts: Vec<String>,
    /// The colour modes the skin supports: dark, light.
    pub themes: Vec<String>,
    /// Whether the desktop app has synced this skin to the user's cloud library.
    pub synced: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinIssueView {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CreateSkinRequest {
    /// The skin's folder name, which must also be the manifest's `id`: lowercase ASCII letters, digits, `_`, `-` and `.`, starting with a letter or digit, at most 64 bytes, and not a built-in theme name.
    pub package_id: String,
    /// The whole `skin.toml`, at most 64 KiB.
    pub manifest: String,
    /// Each image the manifest references (`preview`, `candidate_window.decoration.image`, `candidate_window.background.image`), keyed by its path relative to the skin folder, in standard base64. PNG or JPEG only, each a complete image that decodes with the codec its extension names, at most 3 images, 1 MiB and 2048 pixels on each side each, 2 MiB and 8,000,000 pixels together, the preview at most 256 KiB.
    pub images: BTreeMap<String, String>,
    /// Replace an existing skin of this id whole. Without it an existing skin is left alone and the call refused.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreatedSkin {
    pub id: String,
    /// Whether an existing skin of this id was replaced.
    pub replaced: bool,
}

fn root(state_dir: &Path) -> PathBuf {
    state_dir.join(SKIN_DIRECTORY)
}

pub fn list(state_dir: &Path) -> SkinList {
    let root = root(state_dir);
    let synced = candidate_sync::synced_packages(&state_dir.join(candidate_sync::STATE_FILE));
    let catalog = catalog::scan(&root);
    SkinList {
        skins: catalog
            .packages
            .into_iter()
            .map(|package| SkinView {
                synced: synced.contains_key(&package.id),
                id: package.id,
                name: package.name,
                version: package.version,
                description: package.description,
                layouts: package.layouts,
                themes: package.themes,
            })
            .collect(),
        issues: catalog
            .issues
            .into_iter()
            .map(|issue| SkinIssueView {
                folder: issue.folder,
                reason: issue.reason,
            })
            .collect(),
    }
}

pub fn create(state_dir: &Path, request: &CreateSkinRequest) -> Result<CreatedSkin, String> {
    let root = root(state_dir);
    let replaced =
        request.replace && std::fs::symlink_metadata(root.join(&request.package_id)).is_ok();
    let package = CandidateSkinPackage {
        // Not read by install; the gallery sets it to the publication it downloaded.
        id: Uuid::nil(),
        package_id: request.package_id.clone(),
        manifest: request.manifest.clone(),
        files: request.images.clone(),
    };
    match candidate_community::install(&root, &package, request.replace) {
        Ok(id) => Ok(CreatedSkin { id, replaced }),
        Err("candidate_skin_package") => Err(manifest_problem(request)),
        Err(code) => Err(explain(code).to_owned()),
    }
}

fn explain(code: &str) -> &'static str {
    match code {
        "candidate_skin_exists" => "a skin with this package_id is already installed; list_candidate_skins shows it. Pass replace: true to replace it whole, or choose another id",
        "candidate_skin_file_path" => "images must hold 1 to 3 entries, each a relative path inside the skin folder other than skin.toml, unique ignoring case",
        "candidate_skin_file_type" => "every image must be a .png, .jpg or .jpeg file",
        "candidate_skin_too_large" => "too large: the manifest may be 64 KiB, each image 1 MiB and 2048 pixels on each side, all images 2 MiB and 8,000,000 pixels together, and the preview 256 KiB",
        "candidate_skin_image_invalid" => "an image is not valid standard base64, or its bytes cannot be decoded as the PNG or JPEG its extension names: it may be truncated or corrupted, so generate it again and pass the whole base64",
        "storage" => "the skin folder could not be written",
        _ => "the skin was refused",
    }
}

/// Why `install` refused the manifest, in words an agent can act on. `install` reports every manifest problem with one code, so the manifest is loaded again here, beside empty stand-ins for the images, by the rule the catalog lists skins by.
fn manifest_problem(request: &CreateSkinRequest) -> String {
    if !catalog::is_external_id(&request.package_id) {
        return "package_id must be lowercase ASCII letters, digits, `_`, `-` and `.`, start with a letter or digit, be at most 64 bytes, and not name a built-in theme".into();
    }
    const SHARING_RULES: &str = "a skin must set `preview` to a .png, .jpg or .jpeg image, must not set `toolbar_stylesheet`, and images must hold exactly the images the manifest references";
    let Ok(scratch) = tempfile::tempdir() else {
        return SHARING_RULES.into();
    };
    let folder = scratch.path().join(&request.package_id);
    if std::fs::create_dir(&folder).is_err()
        || std::fs::write(folder.join("skin.toml"), &request.manifest).is_err()
    {
        return SHARING_RULES.into();
    }
    for path in request.images.keys() {
        // Only plain relative paths are stood in for; install already refused anything else with its own code.
        if !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            continue;
        }
        let stand_in = folder.join(path);
        if let Some(parent) = stand_in.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(stand_in, []);
    }
    match catalog::load_package(scratch.path(), &request.package_id) {
        Err(reason) if reason == "manifest id does not match folder" => {
            "the manifest's id must equal package_id".into()
        }
        Err(reason) => format!("skin.toml: {reason}"),
        Ok(_) => SHARING_RULES.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

    fn manifest(id: &str, name: &str) -> String {
        format!(
            "schema_version = 1\nid = '{id}'\nname = '{name}'\nversion = '1.0'\nbase = 'night'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['dark', 'light']\n[candidate_window]\nmin_width_dip = 200\n[candidate.dark]\naccent = '#ff8800'\n"
        )
    }

    fn request(id: &str, name: &str, replace: bool) -> CreateSkinRequest {
        CreateSkinRequest {
            package_id: id.into(),
            manifest: manifest(id, name),
            images: BTreeMap::from([("preview.png".into(), PNG.into())]),
            replace,
        }
    }

    #[test]
    fn a_created_skin_is_listed_as_the_catalog_reads_it() {
        let state = tempfile::tempdir().unwrap();
        assert_eq!(
            create(state.path(), &request("sunset", "Sunset", false)).unwrap(),
            CreatedSkin {
                id: "sunset".into(),
                replaced: false
            }
        );
        assert!(state.path().join("skins/sunset/preview.png").is_file());
        let listed = list(state.path());
        assert!(listed.issues.is_empty());
        assert_eq!(
            listed.skins,
            [SkinView {
                id: "sunset".into(),
                name: "Sunset".into(),
                version: "1.0".into(),
                description: None,
                layouts: vec!["horizontal".into(), "vertical".into()],
                themes: vec!["dark".into(), "light".into()],
                synced: false,
            }]
        );
    }

    #[test]
    fn an_existing_skin_is_replaced_only_when_asked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), &request("sunset", "Sunset", false)).unwrap();
        let refused = create(state.path(), &request("sunset", "Dusk", false)).unwrap_err();
        assert!(refused.contains("replace: true"), "{refused}");
        assert_eq!(list(state.path()).skins[0].name, "Sunset");
        assert!(
            create(state.path(), &request("sunset", "Dusk", true))
                .unwrap()
                .replaced
        );
        assert_eq!(list(state.path()).skins[0].name, "Dusk");
    }

    #[test]
    fn a_refused_manifest_says_what_is_wrong() {
        let state = tempfile::tempdir().unwrap();
        let mut mismatched = request("sunset", "Sunset", false);
        mismatched.package_id = "dusk".into();
        assert!(create(state.path(), &mismatched)
            .unwrap_err()
            .contains("must equal package_id"));

        let mut unsupported = request("sunset", "Sunset", false);
        unsupported.manifest = unsupported.manifest.replace("'night'", "'custom'");
        assert_eq!(
            create(state.path(), &unsupported).unwrap_err(),
            "skin.toml: base must be system or a built-in theme"
        );

        let mut no_preview = request("sunset", "Sunset", false);
        no_preview.manifest = no_preview.manifest.replace("preview = 'preview.png'\n", "");
        assert!(create(state.path(), &no_preview)
            .unwrap_err()
            .contains("must set `preview`"));

        let mut reserved = request("night", "Night", false);
        reserved.manifest = manifest("night", "Night");
        assert!(create(state.path(), &reserved)
            .unwrap_err()
            .contains("built-in theme"));

        let mut not_png = request("sunset", "Sunset", false);
        not_png.images = BTreeMap::from([("preview.png".into(), "bm90IGEgcG5n".into())]);
        assert!(create(state.path(), &not_png)
            .unwrap_err()
            .contains("cannot be decoded"));

        // 签名和 IHDR 完好、像素数据和 IEND 都缺失：只看签名的检查会把它当成 PNG 装进去。
        let mut truncated = request("sunset", "Sunset", false);
        truncated.images = BTreeMap::from([("preview.png".into(), PNG[..44].into())]);
        assert!(create(state.path(), &truncated)
            .unwrap_err()
            .contains("cannot be decoded as the PNG or JPEG"));

        assert!(list(state.path()).skins.is_empty());
    }

    #[test]
    fn a_synced_skin_is_marked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), &request("sunset", "Sunset", false)).unwrap();
        std::fs::write(
            state.path().join(candidate_sync::STATE_FILE),
            serde_json::to_vec(&serde_json::json!({
                "user_id": "user",
                "packages": { "sunset": { "cloud_id": Uuid::new_v4(), "cloud_digest": "a", "local_digest": "b" } }
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(list(state.path()).skins[0].synced);
    }
}
