//! Safe discovery and validation of external candidate-skin manifests.
//!
//! `read_resource` supplies bytes for host resource delivery analogous to the
//! Windows `candidate-skins` virtual-folder mapping (settings_app.cpp at
//! 04a8df56f86312474a069f4335a1b58da7afaa9e). It does not register a protocol,
//! authorize a webview origin, sanitize CSS/SVG, or execute resource content.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use toml::Value;

/// Manifest values, not trusted CSS. Renderers must validate color syntax before
/// inserting these strings into styles; scanning does not authorize CSS execution.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct CandidatePalette {
    pub accent: Option<String>,
    pub selected: Option<String>,
    pub hover: Option<String>,
    pub surface: Option<String>,
    pub border: Option<String>,
    pub text: Option<String>,
    pub number: Option<String>,
    /// Secondary candidate text (translations, comments); the resolved theme's `secondary` slot.
    pub translation: Option<String>,
    #[serde(rename(serialize = "showSelectedBar", deserialize = "show_selected_bar"))]
    pub show_selected_bar: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct CandidateColors {
    pub dark: CandidatePalette,
    pub light: CandidatePalette,
}

fn read_colors(table: &toml::map::Map<String, Value>) -> Result<CandidateColors, String> {
    let Some(value) = table.get("candidate") else {
        return Ok(CandidateColors::default());
    };
    let candidate = value.as_table().ok_or("invalid candidate colors")?;
    for theme in ["dark", "light"] {
        if candidate
            .get(theme)
            .is_some_and(|palette| !palette.is_table())
        {
            return Err("invalid candidate colors".into());
        }
    }
    let colors: CandidateColors = value
        .clone()
        .try_into()
        .map_err(|_| "invalid candidate colors")?;
    for palette in [&colors.dark, &colors.light] {
        for color in [
            &palette.accent,
            &palette.selected,
            &palette.hover,
            &palette.surface,
            &palette.border,
            &palette.text,
            &palette.number,
            &palette.translation,
        ]
        .into_iter()
        .flatten()
        {
            if color.len() > 80 {
                return Err("candidate color exceeds 80 bytes".into());
            }
        }
    }
    Ok(colors)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The global theme the package's colours are drawn over: `system` (the host's native tokens) or a built-in theme. A manifest `base` that names anything else, `custom` included, is a manifest error and the package is not listed.
    pub base: super::theme::GlobalTheme,
    pub author: Option<String>,
    pub description: Option<String>,
    pub layouts: Vec<String>,
    pub themes: Vec<String>,
    pub min_width_dip: f64,
    /// The candidate card's corner radius (`candidate_window.corner_radius_dip`, 0-32); `None` keeps the host's own radius.
    pub corner_radius_dip: Option<f64>,
    pub decoration_top_dip: f64,
    pub decoration_width_dip: f64,
    /// The image drawn in the decoration band, relative to the package: `candidate_window.decoration.image`, or else the package `preview` when that is an image. Set only for a decorated package (both decoration sizes above zero).
    pub decoration_image: Option<String>,
    /// Where the decoration sits along the card's top edge.
    pub decoration_align: DecorationAlign,
    /// An image drawn over the card surface and under the candidates, clipped to the card's rounded outline.
    pub background: Option<SkinBackground>,
    /// The floating toolbar's own geometry and colours, drawn over what the base theme derives and under `toolbar_stylesheet`.
    pub toolbar: SkinToolbar,
    pub toolbar_stylesheet: Option<String>,
    pub preview: Option<String>,
    pub candidate: CandidateColors,
    /// Licence metadata the manifest declares for its code and assets. Informational only: nothing is enforced from it.
    pub license: Option<SkinLicense>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DecorationAlign {
    Left,
    Center,
    /// The trailing edge, which is where every host drew the decoration before the manifest could say otherwise.
    #[default]
    Right,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundFit {
    /// Fill the card, cropping the image's overflow.
    #[default]
    Cover,
    /// Show the whole image inside the card.
    Contain,
    /// Scale each axis to the card independently.
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinBackground {
    /// Relative to the package, an image type, confined to the package directory.
    pub image: String,
    pub fit: BackgroundFit,
    /// 0-1, multiplied into the image.
    pub opacity: f64,
}

/// Toolbar colours for one mode, each normalized by `theme::normalized_color` (`#RRGGBB` or `#RRGGBBAA`); a colour it cannot read is left out, as candidate colours are.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct ToolbarPalette {
    pub background: Option<String>,
    pub border: Option<String>,
    /// The drag handle.
    pub handle: Option<String>,
    pub divider: Option<String>,
    pub icon: Option<String>,
    /// Hovered button fill.
    pub hover: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbar {
    /// 0-32; `None` keeps the host's own radius.
    pub corner_radius_dip: Option<f64>,
    pub dark: ToolbarPalette,
    pub light: ToolbarPalette,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct SkinLicense {
    pub code: Option<String>,
    pub assets: Option<String>,
    pub source: Option<String>,
}

impl SkinSummary {
    fn supports_theme(&self, theme: &str) -> bool {
        self.themes.iter().any(|value| value == theme)
    }

    /// Compatibility comes from the manifest, not the base skin's capabilities.
    pub fn supports(&self, layout: &str, theme: &str) -> bool {
        self.layouts.iter().any(|value| value == layout) && self.supports_theme(theme)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkinIssue {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct SkinCatalog {
    pub packages: Vec<SkinSummary>,
    pub issues: Vec<SkinIssue>,
}

/// Whether `id` is taken by a global theme. External packages may not use these ids, so a theme id and a package folder can never be confused.
pub fn is_reserved(id: &str) -> bool {
    super::theme::GlobalTheme::from_id(id).is_some()
}

/// 该名字能否作为外部皮肤的文件夹名。目录扫描只列出这样的文件夹，导入时按同一规则把关，免得拷进来一个扫描随后拒绝的皮肤。
pub fn is_external_id(id: &str) -> bool {
    safe_id(id) && !is_reserved(id)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && crate::is_ascii_lowercase_identifier_with_dots(id)
}

fn contained(root: &Path, child: &Path) -> bool {
    root.canonicalize()
        .ok()
        .and_then(|r| child.canonicalize().ok().map(|c| c.starts_with(r)))
        .unwrap_or(false)
}

fn required_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<String, String> {
    let value = table
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))?;
    validate_string(value, key, max)?;
    Ok(value.to_owned())
}

fn optional_string(
    table: &toml::map::Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))?;
    validate_string(value, key, max)?;
    Ok(Some(value.to_owned()))
}

fn validate_string(value: &str, key: &str, max: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max {
        return Err(format!("{key} has invalid length"));
    }
    Ok(())
}

fn safe_resource(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && crate::is_ascii_identifier_with_dots(part)
        })
}

fn enum_array(
    table: &toml::map::Map<String, Value>,
    key: &str,
    allowed: &[&str],
) -> Option<Vec<String>> {
    let items = table.get(key)?.as_array()?;
    if items.is_empty() {
        return None;
    }
    let mut values = Vec::with_capacity(items.len().min(allowed.len()));
    for item in items {
        let value = item.as_str()?;
        if !allowed.contains(&value) || values.iter().any(|existing| existing == value) {
            return None;
        }
        values.push(value.to_owned());
    }
    Some(values)
}

fn load(root: &Path, folder: &str) -> Result<SkinSummary, String> {
    if !safe_id(folder) || is_reserved(folder) {
        return Err("invalid skin id".into());
    }
    let dir = root.join(folder);
    let manifest = dir.join("skin.toml");
    if !contained(root, &dir) || !contained(&dir, &manifest) {
        return Err("manifest escapes skin directory".into());
    }
    // Check the type before opening: opening a FIFO for reading blocks until a writer appears, which would stall the scan and every host that resolves a skin.
    if !fs::symlink_metadata(&manifest)
        .map_err(|_| "missing skin.toml".to_owned())?
        .is_file()
    {
        return Err("skin.toml is not a regular file".into());
    }
    let input = fs::File::open(&manifest).map_err(|_| "missing skin.toml".to_owned())?;
    if !input
        .metadata()
        .map_err(|_| "unreadable skin.toml")?
        .is_file()
    {
        return Err("skin.toml is not a regular file".into());
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        input,
        65_536,
        || "skin.toml is too large".to_owned(),
        |_| "unreadable skin.toml".to_owned(),
    )?;
    let value: Value =
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| "skin.toml is not UTF-8")?)
            .map_err(|_| "invalid TOML")?;
    let table = value.as_table().ok_or("manifest must be a table")?;
    if table.get("schema_version").and_then(Value::as_integer) != Some(1) {
        return Err("unsupported schema_version".into());
    }
    let id = required_string(table, "id", 64)?;
    if id != folder || !safe_id(&id) {
        return Err("manifest id does not match folder".into());
    }
    let name = required_string(table, "name", 80)?;
    let version = required_string(table, "version", 32)?;
    let base = required_string(table, "base", 32)?;
    let author = optional_string(table, "author", 120)?;
    let description = optional_string(table, "description", 500)?;
    // msime-windows only accepts its own built-in looks as a base, and the packages written for it (github.com/metasequoiaime/msime-skins) say `fluent`: the native Windows Fluent tokens, which is what `system` draws. Only the manifest reads it that way; everywhere else `fluent` is a retired id and is refused.
    let base = if base == "fluent" {
        "system"
    } else {
        base.as_str()
    };
    let base = super::theme::GlobalTheme::from_id(base)
        .filter(|theme| theme.is_base())
        .ok_or("base must be system or a built-in theme")?;
    let supports = table
        .get("supports")
        .and_then(Value::as_table)
        .ok_or("missing supports")?;
    let layouts =
        enum_array(supports, "layouts", &["horizontal", "vertical"]).ok_or("invalid supports")?;
    let themes = enum_array(supports, "themes", &["dark", "light"]).ok_or("invalid supports")?;
    let window = table
        .get("candidate_window")
        .and_then(Value::as_table)
        .ok_or("missing candidate_window")?;
    let number = |value: Option<&Value>| match value {
        None => 0.0,
        Some(value) => value
            .as_float()
            .or_else(|| value.as_integer().map(|n| n as f64))
            .unwrap_or(f64::NAN),
    };
    let min_width = number(window.get("min_width_dip"));
    if !min_width.is_finite() || !(0.0..=1000.0).contains(&min_width) {
        return Err("invalid min_width_dip".into());
    }
    let corner_radius = match window.get("corner_radius_dip") {
        None => None,
        Some(value) => Some(number_in(value, 0.0..=32.0).ok_or("invalid corner_radius_dip")?),
    };
    // A package without a decoration may leave the whole table out.
    let empty = toml::map::Map::new();
    let decoration = match window.get("decoration") {
        None => &empty,
        Some(value) => value.as_table().ok_or("invalid decoration")?,
    };
    let top = number(decoration.get("top_inset_dip"));
    let width = number(decoration.get("width_dip"));
    if !top.is_finite()
        || !width.is_finite()
        || !(0.0..=500.0).contains(&top)
        || !(0.0..=1000.0).contains(&width)
        || (top == 0.0) != (width == 0.0)
    {
        return Err("invalid decoration".into());
    }
    let decorated = top > 0.0;
    let decoration_image = optional_string(decoration, "image", 256)?;
    if let Some(image) = &decoration_image {
        if !decorated || !image_resource(&dir, image) {
            return Err("invalid decoration".into());
        }
    }
    let decoration_align = match optional_string(decoration, "align", 16)?.as_deref() {
        None | Some("right") => DecorationAlign::Right,
        Some("left") => DecorationAlign::Left,
        Some("center") => DecorationAlign::Center,
        Some(_) => return Err("invalid decoration".into()),
    };
    let background = match window.get("background") {
        None => None,
        Some(value) => Some(read_background(
            &dir,
            value.as_table().ok_or("invalid background")?,
        )?),
    };
    let toolbar = read_toolbar(table)?;
    let license = match table.get("license") {
        None => None,
        Some(value) => {
            let license = value.as_table().ok_or("invalid license")?;
            Some(SkinLicense {
                code: optional_string(license, "code", 120)?,
                assets: optional_string(license, "assets", 120)?,
                source: optional_string(license, "source", 500)?,
            })
        }
    };
    let toolbar_stylesheet = optional_string(table, "toolbar_stylesheet", 128)?;
    if let Some(stylesheet) = &toolbar_stylesheet {
        if !safe_resource(stylesheet, 128)
            || stylesheet.contains('/')
            || stylesheet.len() <= 4
            || !stylesheet.ends_with(".css")
            || !contained(&dir, &dir.join(stylesheet))
            || !dir.join(stylesheet).is_file()
        {
            return Err("invalid toolbar_stylesheet".into());
        }
    }
    let preview = optional_string(table, "preview", 256)?;
    if let Some(preview) = &preview {
        if !safe_resource(preview, 256) || !contained(&dir, &dir.join(preview)) {
            return Err("invalid preview".into());
        }
    }
    let candidate = read_colors(table)?;
    let decoration_image = decoration_image.or_else(|| {
        preview
            .clone()
            .filter(|preview| decorated && is_image(preview))
    });
    Ok(SkinSummary {
        id,
        name,
        version,
        base,
        author,
        description,
        layouts,
        themes,
        min_width_dip: min_width,
        corner_radius_dip: corner_radius,
        decoration_top_dip: top,
        decoration_width_dip: width,
        decoration_image,
        decoration_align,
        background,
        toolbar,
        toolbar_stylesheet,
        preview,
        candidate,
        license,
    })
}

/// A TOML integer or float inside `range`.
fn number_in(value: &Value, range: std::ops::RangeInclusive<f64>) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
        .filter(|number| number.is_finite() && range.contains(number))
}

fn is_image(relative: &str) -> bool {
    resource_content_type(relative).is_some_and(|kind| kind.starts_with("image/"))
}

/// A package-relative image that exists inside the package directory.
fn image_resource(dir: &Path, relative: &str) -> bool {
    safe_resource(relative, 256) && is_image(relative) && contained(dir, &dir.join(relative))
}

fn read_background(
    dir: &Path,
    table: &toml::map::Map<String, Value>,
) -> Result<SkinBackground, String> {
    let invalid = || "invalid background".to_owned();
    let image = table
        .get("image")
        .and_then(Value::as_str)
        .filter(|image| image_resource(dir, image))
        .ok_or_else(invalid)?
        .to_owned();
    let fit = match table.get("fit") {
        None => BackgroundFit::Cover,
        Some(value) => match value.as_str().ok_or_else(invalid)? {
            "cover" => BackgroundFit::Cover,
            "contain" => BackgroundFit::Contain,
            "stretch" => BackgroundFit::Stretch,
            _ => return Err(invalid()),
        },
    };
    let opacity = match table.get("opacity") {
        None => 1.0,
        Some(value) => number_in(value, 0.0..=1.0).ok_or_else(invalid)?,
    };
    Ok(SkinBackground {
        image,
        fit,
        opacity,
    })
}

fn read_toolbar(table: &toml::map::Map<String, Value>) -> Result<SkinToolbar, String> {
    let invalid = || "invalid toolbar".to_owned();
    let Some(value) = table.get("toolbar") else {
        return Ok(SkinToolbar::default());
    };
    let toolbar = value.as_table().ok_or_else(invalid)?;
    let corner_radius_dip = match toolbar.get("corner_radius_dip") {
        None => None,
        Some(value) => Some(number_in(value, 0.0..=32.0).ok_or_else(invalid)?),
    };
    let palette = |mode: &str| -> Result<ToolbarPalette, String> {
        let Some(value) = toolbar.get(mode) else {
            return Ok(ToolbarPalette::default());
        };
        let colors = value.as_table().ok_or_else(invalid)?;
        let color = |key: &str| -> Result<Option<String>, String> {
            let Some(value) = colors.get(key) else {
                return Ok(None);
            };
            let value = value.as_str().ok_or_else(invalid)?;
            if value.len() > 80 {
                return Err("toolbar color exceeds 80 bytes".into());
            }
            Ok(super::theme::normalized_color(value))
        };
        Ok(ToolbarPalette {
            background: color("background")?,
            border: color("border")?,
            handle: color("handle")?,
            divider: color("divider")?,
            icon: color("icon")?,
            hover: color("hover")?,
        })
    };
    Ok(SkinToolbar {
        corner_radius_dip,
        dark: palette("dark")?,
        light: palette("light")?,
    })
}

/// Maximum bytes returned for one skin asset, including stylesheets and fonts.
pub const MAX_RESOURCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ResourceError {
    InvalidPath,
    InvalidPackage,
    UnsupportedType,
    Unavailable,
    TooLarge,
    InvalidEncoding,
}

/// Read only the toolbar stylesheet declared by the current manifest. The
/// caller chooses a package, not a filesystem path. None means inheritance of
/// the built-in toolbar; an empty stylesheet is Some(""). Returned CSS is
/// untrusted and must be parsed/scoped by the host before applying it.
pub fn read_toolbar_stylesheet(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<Option<String>, ResourceError> {
    if !safe_id(id) {
        return Err(ResourceError::InvalidPath);
    }
    let root = root.as_ref();
    let directory = root.join(id);
    if !fs::symlink_metadata(directory)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        return Err(ResourceError::InvalidPackage);
    }
    let package = load(root, id).map_err(|_| ResourceError::InvalidPackage)?;
    let Some(relative) = package.toolbar_stylesheet else {
        return Ok(None);
    };
    read_stylesheet(root, id, &relative).map(Some)
}

/// Read one package-local CSS resource. This is also the boundary used for
/// relative `@import` rules: callers never submit a filesystem path, and every
/// imported sheet is revalidated against the current manifest and package.
pub fn read_stylesheet(
    root: impl AsRef<Path>,
    id: &str,
    relative: &str,
) -> Result<String, ResourceError> {
    let resource = read_resource(root, id, relative)?;
    if resource.content_type != "text/css; charset=utf-8" {
        return Err(ResourceError::UnsupportedType);
    }
    let text = String::from_utf8(resource.bytes).map_err(|_| ResourceError::InvalidEncoding)?;
    // A UTF-8 BOM is an encoding marker, not part of the first selector.
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

/// Untrusted resource data. Hosts must set the content type, disable MIME
/// sniffing, and isolate styles/SVG rather than inserting them as page markup.
#[derive(Debug, PartialEq, Eq)]
pub struct SkinResource {
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The content type a package resource is served with, by extension; `None` for a type packages may not ship.
fn resource_content_type(relative: &str) -> Option<&'static str> {
    Some(
        match relative
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "css" => "text/css; charset=utf-8",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "bmp" => "image/bmp",
            "avif" => "image/avif",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            _ => return None,
        },
    )
}

/// Read an asset from a currently valid package under a host-selected root.
/// Paths are decoded relative names, never URLs. Canonical containment rejects
/// symlink escapes, but is not a sandbox against concurrent hostile filesystem
/// mutation; hosts must not use this API to expose attacker-writable trees
/// across a privilege boundary.
pub fn read_resource(
    root: impl AsRef<Path>,
    id: &str,
    relative: &str,
) -> Result<SkinResource, ResourceError> {
    if !safe_id(id) || !safe_resource(relative, 256) {
        return Err(ResourceError::InvalidPath);
    }
    let content_type = resource_content_type(relative).ok_or(ResourceError::UnsupportedType)?;
    let root = root.as_ref();
    let directory = root.join(id);
    // Match scan(): symlinked package directories are not catalog entries.
    if !fs::symlink_metadata(&directory)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
        || load(root, id).is_err()
    {
        return Err(ResourceError::InvalidPackage);
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| ResourceError::Unavailable)?;
    let target = directory
        .join(relative)
        .canonicalize()
        .map_err(|_| ResourceError::Unavailable)?;
    if !target.starts_with(&directory) {
        return Err(ResourceError::InvalidPath);
    }
    if !target.is_file() {
        return Err(ResourceError::Unavailable);
    }
    let input = fs::File::open(&target).map_err(|_| ResourceError::Unavailable)?;
    let metadata = input.metadata().map_err(|_| ResourceError::Unavailable)?;
    if !metadata.is_file() {
        return Err(ResourceError::Unavailable);
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        input,
        MAX_RESOURCE_BYTES as u64,
        || ResourceError::TooLarge,
        |_| ResourceError::Unavailable,
    )?;
    Ok(SkinResource {
        content_type,
        bytes,
    })
}

/// Validate one installed package with the same rules `scan` applies, without reading the rest of the root. Native presenters use this to resolve the selected skin, so a package the settings page lists as valid is the one they draw. Like `scan`, a symlinked package directory is not a package.
pub fn load_package(root: impl AsRef<Path>, id: &str) -> Result<SkinSummary, String> {
    let root = root.as_ref();
    if !safe_id(id) || is_reserved(id) {
        return Err("invalid skin id".into());
    }
    if !fs::symlink_metadata(root.join(id))
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        return Err("missing skin directory".into());
    }
    load(root, id)
}

pub fn scan(root: impl AsRef<Path>) -> SkinCatalog {
    let root = root.as_ref();
    let mut out = SkinCatalog::default();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        match load(root, &folder) {
            Ok(package) => out.packages.push(package),
            Err(reason) => out.issues.push(SkinIssue { folder, reason }),
        }
    }
    out.packages.sort_by(|a, b| a.name.cmp(&b.name));
    out.issues.sort_by(|a, b| a.folder.cmp(&b.folder));
    out
}

/// Most installed skins published to hosts that draw the candidate panel from `candidate_skin_catalog` (the Linux IBus and Fcitx5 hosts). Beyond this a skin menu is no longer usable, and every entry costs the document those hosts read whole.
pub const HOST_CATALOG_MAX_PACKAGES: usize = 32;

/// The installed skins in the shape a native candidate host reads (`candidate_skin_catalog` in the Linux runtime options): the id, the manifest name as `title`, the `base` global theme id, the declared `layouts`, per declared theme (an empty object when it sets no colour) every candidate colour `theme::resolve` reads (surface, border, text, number, accent, selected, hover, translation, each normalized by `theme::normalized_color`) and `show_selected_bar`, the card's `corner_radius_dip` when the manifest sets one, and for a package that declares a decoration its size and the absolute path of the image drawn there (`decoration_top_dip`, `decoration_width_dip`, `decoration_image`), plus `decoration_align` when it is not the default `right`. `root` is the directory `catalog` was scanned from. Passing an entry to `msime_client_resolve_theme` as `package` therefore resolves exactly as reading the same package from the skin root does.
///
/// Everything else in a package - the background image, the toolbar, other paths and stylesheets - stays out: the Linux hosts cannot draw them and every byte counts against the size limit of the document it reads, which is also why an undecorated package carries no decoration keys at all. A colour `normalized_color` cannot read is left out, as `resolve` ignores it. A palette is kept only for a theme the package declares, since every host drops an external skin for a layout or theme it does not support rather than drawing its colours there, and `resolve` does the same with the published `layouts` and themes. Ids and names are already bounded by `scan`; the hosts re-check both, and the decoration's bounds.
///
/// At most `HOST_CATALOG_MAX_PACKAGES` are listed, in the catalog's order. The `selected` skin is always among them when installed, taking the last place if it falls beyond the cap, because its colours are the ones on screen.
pub fn host_candidate_catalog(
    catalog: &SkinCatalog,
    root: &Path,
    selected: &str,
) -> serde_json::Value {
    let mut listed = Vec::with_capacity(HOST_CATALOG_MAX_PACKAGES + 1);
    listed.extend(
        catalog
            .packages
            .iter()
            .enumerate()
            .filter(|(index, package)| *index < HOST_CATALOG_MAX_PACKAGES || package.id == selected)
            .map(|(_, package)| package),
    );
    if listed.len() > HOST_CATALOG_MAX_PACKAGES {
        listed.remove(HOST_CATALOG_MAX_PACKAGES - 1);
    }
    let mut packages = Vec::with_capacity(HOST_CATALOG_MAX_PACKAGES);
    packages.extend(listed.into_iter().map(|package| {
            let mut entry = serde_json::json!({ "id": package.id, "title": package.name, "base": package.base, "layouts": package.layouts });
            let mut candidate = serde_json::Map::new();
            for (theme, palette) in [
                ("light", &package.candidate.light),
                ("dark", &package.candidate.dark),
            ] {
                if !package.supports_theme(theme) {
                    continue;
                }
                // A declared mode is published even with no colour, so the entry still says the package may be drawn in it.
                candidate.insert(
                    theme.to_owned(),
                    serde_json::Value::Object(host_palette(palette)),
                );
            }
            if !candidate.is_empty() {
                entry["candidate"] = serde_json::Value::Object(candidate);
            }
            if let Some(image) = host_decoration_image(root, package) {
                entry["decoration_top_dip"] = package.decoration_top_dip.into();
                entry["decoration_width_dip"] = package.decoration_width_dip.into();
                entry["decoration_image"] = image.into();
                if package.decoration_align != DecorationAlign::Right {
                    entry["decoration_align"] = serde_json::to_value(package.decoration_align)
                        .expect("an enum serializes");
                }
            }
            if let Some(radius) = package.corner_radius_dip {
                entry["corner_radius_dip"] = radius.into();
            }
            entry
        }));
    serde_json::json!({ "packages": packages })
}

/// The image a decorated package draws above its candidate window: its `decoration_image` (the manifest's `decoration.image`, or else its preview, as on Windows where candidate_presenter.cpp draws `<skins>/<id>/<preview>` in a band `decoration_top_dip` tall). `scan` has already confined it to the package directory and checked it is an image. No decoration is published when the path would not be absolute or not UTF-8, since the host could not open it.
fn host_decoration_image(root: &Path, package: &SkinSummary) -> Option<String> {
    let image = package.decoration_image.as_deref()?;
    let path = root.join(&package.id).join(image);
    if !path.is_absolute() {
        return None;
    }
    path.to_str().map(str::to_owned)
}

fn host_palette(palette: &CandidatePalette) -> serde_json::Map<String, serde_json::Value> {
    let mut colors = serde_json::Map::new();
    for (key, value) in [
        ("surface", &palette.surface),
        ("border", &palette.border),
        ("text", &palette.text),
        ("number", &palette.number),
        ("accent", &palette.accent),
        ("selected", &palette.selected),
        ("hover", &palette.hover),
        ("translation", &palette.translation),
    ] {
        if let Some(value) = value.as_deref().and_then(super::theme::normalized_color) {
            colors.insert(key.to_owned(), value.into());
        }
    }
    if let Some(value) = palette.show_selected_bar {
        colors.insert("show_selected_bar".to_owned(), value.into());
    }
    colors
}

#[cfg(test)]
mod tests;
