//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;
use std::io::Cursor;
use tempfile::tempdir;
fn manifest(id: &str) -> String {
    format!("schema_version = 1\nid = '{id}'\nname = 'Sample'\nversion = '1.0'\nbase = 'night'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n")
}

fn resource_package(root: &Path) -> std::path::PathBuf {
    let skin = root.join("sample");
    fs::create_dir_all(skin.join("images")).unwrap();
    fs::write(skin.join("skin.toml"), manifest("sample")).unwrap();
    skin
}

fn encoded_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb([0, 0, 0]));
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut output, image::ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn encoded_image(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([0, 0, 0, 255]),
    ));
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, format).unwrap();
    output.into_inner()
}

fn encoded_gif(width: u32, height: u32, frames: u32) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    let mut encoder = image::codecs::gif::GifEncoder::new(&mut output);
    encoder
        .encode_frames((0..frames).map(|frame| {
            image::Frame::new(image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([(frame * 40 % 256) as u8, 0, 0, 255]),
            ))
        }))
        .unwrap();
    drop(encoder);
    output.into_inner()
}

/// 一张 `frames` 帧、`width`×`height` 的 APNG，每帧颜色不同，无限循环。
fn encoded_apng(width: u32, height: u32, frames: u32) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_animated(frames, 0).unwrap();
    encoder.set_frame_delay(1, 10).unwrap();
    let mut writer = encoder.write_header().unwrap();
    for frame in 0..frames {
        let pixel = [(frame * 40 % 256) as u8, 0, 0, 255];
        writer
            .write_image_data(&pixel.repeat((width * height) as usize))
            .unwrap();
    }
    writer.finish().unwrap();
    output
}

/// 带装饰图的 schema 2 包，装饰图是 `decoration`，`motion` 是写进 `[candidate_window.decoration]` 的额外几行。
fn decorated_package(root: &Path, decoration: &[u8], motion: &str) {
    let skin = root.join("whale");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("whale.png"), decoration).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("schema_version = 2\nid = 'whale'\nname = 'Whale'\nversion = '1.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = 'whale.png'\ntop_inset_dip = 40\nwidth_dip = 60\n{motion}"),
    )
    .unwrap();
}

#[test]
fn decoration_motion_and_frames_reach_the_hosts() {
    let root = tempdir().unwrap();
    decorated_package(
        root.path(),
        &encoded_png(6, 4),
        "motion = 'sway'\nmotion_period_ms = 2400\n",
    );
    let package = load(root.path(), "whale").unwrap();
    assert_eq!(package.decoration_motion, DecorationMotion::Sway);
    assert_eq!(package.decoration_motion_period_ms, 2400);
    assert_eq!(package.decoration_frames, 1);
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["decorationMotion"], "sway");
    assert_eq!(json["decorationMotionPeriodMs"], 2400);
    assert_eq!(json["decorationFrames"], 1);

    // 只写动效不写周期时用默认周期；APNG 报出 acTL 的帧数。
    let root = tempdir().unwrap();
    decorated_package(root.path(), &encoded_apng(6, 4, 12), "motion = 'float'\n");
    let package = load(root.path(), "whale").unwrap();
    assert_eq!(package.decoration_motion, DecorationMotion::Float);
    assert_eq!(
        package.decoration_motion_period_ms,
        DEFAULT_DECORATION_MOTION_PERIOD_MS
    );
    assert_eq!(package.decoration_frames, 12);
    assert_eq!(png_frames(&encoded_apng(6, 4, 12)), Ok((12, 12)));
    assert_eq!(png_frames(&encoded_png(6, 4)), Ok((1, 1)));
}

#[test]
fn windows_skin_v2_fields_and_wechat_scroll_base_are_preserved() {
    let root = tempdir().unwrap();
    let skin = root.path().join("canvas_skin");
    fs::create_dir(&skin).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 2
id = 'canvas_skin'
name = 'Canvas skin'
version = '1'
base = 'canvas'
[supports]
layouts = ['horizontal', 'vertical']
themes = ['dark', 'light']
[candidate_window]
backdrop = 'mica'
font_family = '霞鹜文楷'
page_arrows = false
fit_content = true
expand_panel = true
expand_rows = 4
padding = '0.5em'
item_gap = 2
selected_bar_height_em = 0.8
preedit_margin_dip = [1, 2]
[candidate_window.horizontal]
justify_rows = true
[candidate_window.content_insets_dip]
top = 10
right = 12
bottom = 10
left = 12
[candidate.dark]
candidate_text = '#EDEDED'
preedit_text = '#D0D0D0'
selected_translation = '#FFFFFFCC'
backdrop_opacity = 0.4
backdrop_luminosity = 0.9
[candidate.dark.menu]
background = '#202020'
border = '#303030'
text = '#FFFFFF'
hover = '#404040'

",
    )
    .unwrap();
    let package = load(root.path(), "canvas_skin").unwrap();
    assert!(package.canvas);
    assert_eq!(package.backdrop.as_deref(), Some("mica"));
    assert_eq!(package.font_family.as_deref(), Some("霞鹜文楷"));
    assert_eq!(package.expand_rows, Some(4));
    assert_eq!(
        package.geometry.padding,
        Some(SkinLength {
            value: 0.5,
            em: true
        })
    );
    assert_eq!(
        package.geometry.preedit_margin_dip,
        Some(SkinInsets {
            top: 1.0,
            right: 2.0,
            bottom: 1.0,
            left: 2.0
        })
    );
    assert!(package.horizontal.justify_rows == Some(true));
    assert_eq!(package.content_insets_dip.unwrap().left, 12.0);
    assert_eq!(package.candidate.dark.backdrop_opacity, Some(0.4));
    assert_eq!(
        package
            .candidate
            .dark
            .menu
            .as_ref()
            .unwrap()
            .hover
            .as_deref(),
        Some("#404040")
    );

    let scroll = root.path().join("scroll_skin");
    fs::create_dir(&scroll).unwrap();
    fs::write(
        scroll.join("skin.toml"),
        manifest("scroll_skin").replace("base = 'night'", "base = 'wechat_scroll'"),
    )
    .unwrap();
    let package = load(root.path(), "scroll_skin").unwrap();
    assert_eq!(package.base, super::super::theme::GlobalTheme::System);
    assert_eq!(package.candidate.dark.show_selected_bar, Some(false));
}

#[test]
fn canvas_requires_v2_and_reserved_canvas_id_is_not_external() {
    let catalog = scan_manifest(&manifest("sample").replace("base = 'night'", "base = 'canvas'"));
    assert!(catalog.packages.is_empty());
    assert!(catalog.issues[0].reason.contains("schema_version 2"));
    assert!(!is_external_id("canvas"));
    assert!(!is_external_id("wechat_scroll"));
}

#[test]
fn animated_decorations_are_bounded_by_frames_and_decoded_pixels() {
    let root = tempdir().unwrap();
    decorated_package(root.path(), &encoded_apng(2, 2, MAX_ANIMATION_FRAMES), "");
    assert_eq!(
        load(root.path(), "whale").unwrap().decoration_frames,
        MAX_ANIMATION_FRAMES
    );

    let root = tempdir().unwrap();
    decorated_package(
        root.path(),
        &encoded_apng(2, 2, MAX_ANIMATION_FRAMES + 1),
        "",
    );
    assert_eq!(
        load(root.path(), "whale").unwrap_err(),
        ANIMATION_TOO_MANY_FRAMES
    );

    // 每一帧都按整张画布计入包的像素合计：1000×1000 的 9 帧是 900 万像素，超过 800 万；8 帧正好 800 万，照常加载。
    let root = tempdir().unwrap();
    decorated_package(root.path(), &encoded_apng(1000, 1000, 9), "");
    assert_eq!(
        load(root.path(), "whale").unwrap_err(),
        IMAGE_DIMENSIONS_TOO_LARGE
    );
    let root = tempdir().unwrap();
    decorated_package(root.path(), &encoded_apng(1000, 1000, 8), "");
    assert_eq!(load(root.path(), "whale").unwrap().decoration_frames, 8);
}

#[test]
fn gif_and_webp_decorations_are_dimension_checked_and_counted() {
    for (extension, bytes, expected_frames) in [
        ("gif", encoded_gif(6, 4, 3), 3),
        ("webp", encoded_image(6, 4, image::ImageFormat::WebP), 1),
    ] {
        let root = tempdir().unwrap();
        let skin = root.path().join("animated");
        fs::create_dir(&skin).unwrap();
        let image = format!("decoration.{extension}");
        fs::write(skin.join(&image), bytes).unwrap();
        fs::write(
            skin.join("skin.toml"),
            format!(
                "schema_version = 1\nid = 'animated'\nname = 'Animated'\nversion = '1'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = '{image}'\ntop_inset_dip = 10\nwidth_dip = 10\n"
            ),
        )
        .unwrap();
        assert_eq!(
            load(root.path(), "animated").unwrap().decoration_frames,
            expected_frames
        );
    }
}

#[test]
fn catalog_rejects_webp_inner_geometry_instead_of_static_fallback() {
    use crate::skin::webp_preflight::tests::{animated, static_images, with_inner_dimensions};
    let images = static_images();
    assert_eq!(
        decoded_animation_frames(&animated(&images), image::ImageFormat::WebP, (4, 3), 36),
        Ok((3, 3))
    );
    for bytes in images {
        let mismatch = with_inner_dimensions(bytes, 5, 3);
        let bytes = animated(&[mismatch]);
        assert_eq!(
            decoded_animation_frames(&bytes, image::ImageFormat::WebP, (4, 3), 12),
            Err("invalid WebP frame geometry")
        );
        let root = tempdir().unwrap();
        let skin = resource_package(root.path());
        fs::write(skin.join("motion.webp"), bytes).unwrap();
        fs::write(
            skin.join("skin.toml"),
            manifest("sample")
                .replace("top_inset_dip = 0", "top_inset_dip = 10")
                .replace("width_dip = 0", "width_dip = 10")
                .replace(
                    "[candidate_window.decoration]",
                    "[candidate_window.decoration]\nimage = 'motion.webp'",
                ),
        )
        .unwrap();
        assert_eq!(
            load(root.path(), "sample").unwrap_err(),
            "invalid WebP frame geometry"
        );
    }
}

#[test]
fn gif_frames_stop_at_the_frame_limit_and_the_package_budget() {
    assert_eq!(
        decoded_animation_frames(
            &encoded_gif(2, 2, MAX_ANIMATION_FRAMES + 1),
            image::ImageFormat::Gif,
            (2, 2),
            u64::MAX
        ),
        Err(ANIMATION_TOO_MANY_FRAMES)
    );
    // 剩余额度只够两帧半：第三帧解码完就停，不再往下解。
    assert_eq!(
        decoded_animation_frames(
            &encoded_gif(10, 10, 5),
            image::ImageFormat::Gif,
            (10, 10),
            250
        ),
        Err(IMAGE_DIMENSIONS_TOO_LARGE)
    );
    assert_eq!(
        decoded_animation_frames(
            &encoded_gif(10, 10, 5),
            image::ImageFormat::Gif,
            (10, 10),
            500
        ),
        Ok((5, 5))
    );
}

#[test]
fn a_gif_with_a_damaged_tail_counts_the_frames_before_it() {
    let mut bytes = encoded_gif(6, 4, 3);
    assert_eq!(bytes.pop(), Some(0x3B));
    bytes.extend([0x99, 0x00, 0x3B]);
    assert_eq!(
        decoded_animation_frames(&bytes, image::ImageFormat::Gif, (6, 4), u64::MAX),
        Ok((3, 3))
    );
    // 第一帧都读不出的旧资源仍按静态图处理。
    assert_eq!(
        decoded_animation_frames(b"GIF89a", image::ImageFormat::Gif, (6, 4), u64::MAX),
        Ok((1, 1))
    );
}

#[test]
fn a_gif_frame_larger_than_its_screen_is_rejected_before_allocating() {
    // 逻辑画布 64x64，第一帧的描述符却声明 65535x65535：不设上限时解码器会为这一帧申请约 17 GB。
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend([64, 0, 64, 0, 0x80, 0, 0]);
    bytes.extend([0, 0, 0, 255, 255, 255]);
    bytes.extend([0x2C, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
    bytes.extend([0x02, 0x02, 0x44, 0x01, 0x00, 0x3B]);
    assert_eq!(
        decoded_animation_frames(&bytes, image::ImageFormat::Gif, (64, 64), u64::MAX),
        Err(IMAGE_DIMENSIONS_TOO_LARGE)
    );
    let root = tempdir().unwrap();
    let skin = root.path().join("animated");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("decoration.gif"), bytes).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 2\nid = 'animated'\nname = 'Animated'\nversion = '1'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = 'decoration.gif'\ntop_inset_dip = 10\nwidth_dip = 10\n",
    )
    .unwrap();
    assert_eq!(
        load(root.path(), "animated").unwrap_err(),
        IMAGE_DIMENSIONS_TOO_LARGE
    );
}

#[test]
fn gif_decorations_over_the_package_pixel_budget_are_rejected() {
    let root = tempdir().unwrap();
    let skin = root.path().join("animated");
    fs::create_dir(&skin).unwrap();
    // 1000x900 的 9 帧合计 8.1M 像素，超过 MAX_PACKAGE_PIXELS。
    fs::write(skin.join("decoration.gif"), encoded_gif(1000, 900, 9)).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 2\nid = 'animated'\nname = 'Animated'\nversion = '1'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = 'decoration.gif'\ntop_inset_dip = 10\nwidth_dip = 10\n",
    )
    .unwrap();
    assert_eq!(
        load(root.path(), "animated").unwrap_err(),
        IMAGE_DIMENSIONS_TOO_LARGE
    );
}

#[test]
fn scan_external_skin_under_non_ascii_directory() {
    let root = tempdir().unwrap();
    let non_ascii_root = root.path().join("用户目录").join("skins");
    let skin = resource_package(&non_ascii_root);

    let package = load(&non_ascii_root, "sample").unwrap();
    assert_eq!(package.id, "sample");
    assert_eq!(package.name, "Sample");
    assert_eq!(package.base, super::super::theme::GlobalTheme::Night);

    let catalog = scan(&non_ascii_root);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages.len(), 1);
    assert_eq!(catalog.packages[0].id, "sample");
    assert!(skin.join("skin.toml").exists());
}

#[test]
fn load_package_rejects_images_with_oversized_dimensions() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("images/too-wide.png"), encoded_png(2049, 1)).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!(
            "{}[candidate_window.background]\nimage = 'images/too-wide.png'\n",
            manifest("sample")
        ),
    )
    .unwrap();

    assert!(load_package(root.path(), "sample").is_err());
}

#[test]
fn toolbar_source_distinguishes_absent_empty_and_declared_utf8_text() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("undeclared.css"), ".other {}").unwrap();
    assert_eq!(read_toolbar_stylesheet(root.path(), "sample"), Ok(None));
    fs::write(skin.join("toolbar.css"), "").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(String::new()))
    );
    fs::write(
        skin.join("toolbar.css"),
        "\u{feff}.status-bar { color: #123456; } /* 示例 */",
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(".status-bar { color: #123456; } /* 示例 */".into()))
    );
}

#[test]
fn toolbar_source_rechecks_manifest_and_rejects_invalid_encoding_and_size() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("toolbar.css"), [0xff, 0xfe]).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidEncoding)
    );
    fs::File::create(skin.join("toolbar.css"))
        .unwrap()
        .set_len(MAX_RESOURCE_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::TooLarge)
    );
    fs::write(skin.join("replacement.css"), ".replacement {}").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!(
            "toolbar_stylesheet = 'replacement.css'\n{}",
            manifest("sample")
        ),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Ok(Some(".replacement {}".into()))
    );
    fs::write(skin.join("skin.toml"), "invalid").unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidPackage)
    );
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "../sample"),
        Err(ResourceError::InvalidPath)
    );
}

#[cfg(unix)]
#[test]
fn toolbar_source_rejects_package_alias_and_escaping_stylesheet() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("outside.css"), ".outside {}").unwrap();
    symlink(outside.path().join("outside.css"), skin.join("toolbar.css")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("toolbar_stylesheet = 'toolbar.css'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "sample"),
        Err(ResourceError::InvalidPackage)
    );
    symlink(&skin, root.path().join("alias")).unwrap();
    assert_eq!(
        read_toolbar_stylesheet(root.path(), "alias"),
        Err(ResourceError::InvalidPackage)
    );
}

#[test]
fn resource_reader_preserves_bytes_and_assigns_supported_content_types() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    for (extension, content_type) in [
        ("css", "text/css; charset=utf-8"),
        ("PNG", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
        ("ico", "image/x-icon"),
        ("bmp", "image/bmp"),
        ("avif", "image/avif"),
        ("woff", "font/woff"),
        ("woff2", "font/woff2"),
        ("ttf", "font/ttf"),
        ("otf", "font/otf"),
    ] {
        let relative = format!("images/sample.{extension}");
        fs::write(skin.join(&relative), [0, 1, 255]).unwrap();
        let result = read_resource(root.path(), "sample", &relative).unwrap();
        assert_eq!(result.content_type, content_type);
        assert_eq!(result.bytes, [0, 1, 255]);
    }
}

#[test]
fn stylesheet_reader_accepts_only_package_local_utf8_css() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::create_dir_all(skin.join("styles")).unwrap();
    fs::write(
        skin.join("styles/imported.css"),
        b"\xEF\xBB\xBF.imported {}",
    )
    .unwrap();
    fs::write(skin.join("styles/broken.css"), [0xff, 0xfe]).unwrap();
    fs::write(skin.join("styles/image.png"), b"not-an-image-decoder-test").unwrap();
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/imported.css").unwrap(),
        ".imported {}"
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/broken.css"),
        Err(ResourceError::InvalidEncoding)
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "styles/image.png"),
        Err(ResourceError::UnsupportedType)
    );
    assert_eq!(
        read_stylesheet(root.path(), "sample", "../outside.css"),
        Err(ResourceError::InvalidPath)
    );
}

#[test]
fn resource_reader_rejects_paths_urls_and_non_asset_types() {
    let root = tempdir().unwrap();
    resource_package(root.path());
    for relative in [
        "",
        "../sample.png",
        "/sample.png",
        "C:/sample.png",
        "images\\sample.png",
        "images//sample.png",
        "./sample.png",
        "%2e%2e/sample.png",
        "https://example.invalid/a.png",
        "sample.png?x=1",
    ] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::InvalidPath)
        );
    }
    assert_eq!(
        read_resource(root.path(), "../sample", "sample.png"),
        Err(ResourceError::InvalidPath)
    );
    for relative in [
        "skin.toml",
        "code.js",
        "page.html",
        "settings.json",
        "program.exe",
        "unknown",
    ] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::UnsupportedType)
        );
    }
}

#[test]
fn resource_reader_requires_current_valid_manifest() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    fs::write(skin.join("sample.png"), b"synthetic").unwrap();
    assert!(read_resource(root.path(), "sample", "sample.png").is_ok());
    fs::write(skin.join("skin.toml"), "invalid").unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "sample.png"),
        Err(ResourceError::InvalidPackage)
    );
    assert_eq!(
        read_resource(root.path(), "absent", "sample.png"),
        Err(ResourceError::InvalidPackage)
    );
}

#[test]
fn resource_reader_rejects_missing_directory_and_oversized_assets() {
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    assert_eq!(
        read_resource(root.path(), "sample", "absent.png"),
        Err(ResourceError::Unavailable)
    );
    fs::create_dir(skin.join("directory.png")).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "directory.png"),
        Err(ResourceError::Unavailable)
    );
    let file = fs::File::create(skin.join("large.png")).unwrap();
    file.set_len(MAX_RESOURCE_BYTES as u64).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "large.png")
            .unwrap()
            .bytes
            .len(),
        MAX_RESOURCE_BYTES
    );
    file.set_len(MAX_RESOURCE_BYTES as u64 + 1).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "large.png"),
        Err(ResourceError::TooLarge)
    );
}

#[cfg(unix)]
#[test]
fn resource_reader_rejects_symlink_escapes_and_package_aliases() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let skin = resource_package(root.path());
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("sample.png"), b"synthetic").unwrap();
    symlink(outside.path().join("sample.png"), skin.join("escape.png")).unwrap();
    symlink(outside.path(), skin.join("escape")).unwrap();
    for relative in ["escape.png", "escape/sample.png"] {
        assert_eq!(
            read_resource(root.path(), "sample", relative),
            Err(ResourceError::InvalidPath)
        );
    }
    symlink(&skin, root.path().join("alias")).unwrap();
    assert_eq!(
        read_resource(root.path(), "alias", "escape.png"),
        Err(ResourceError::InvalidPackage)
    );
    fs::write(skin.join("images/local.png"), b"local").unwrap();
    symlink(skin.join("images/local.png"), skin.join("local.png")).unwrap();
    assert_eq!(
        read_resource(root.path(), "sample", "local.png")
            .unwrap()
            .bytes,
        b"local"
    );
}

fn scan_manifest(body: &str) -> SkinCatalog {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), body).unwrap();
    scan(root.path())
}

#[test]
fn candidate_palettes_preserve_both_themes_and_serialize_host_names() {
    let body = format!("{}\n[candidate.dark]\naccent = '#123456'\nselected = '#234567'\nhover = '#345678'\nsurface = '#456789'\nborder = '#56789a'\ntext = '#6789ab'\nnumber = '#789abc'\ntranslation = '#89abcd'\nshow_selected_bar = false\n[candidate.light]\ntext = '#123'\nshow_selected_bar = true\n", manifest("sample"));
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let json = serde_json::to_value(&catalog.packages[0]).unwrap();
    assert_eq!(
        json["candidate"]["dark"],
        serde_json::json!({
            "accent": "#123456", "selected": "#234567", "hover": "#345678",
            "surface": "#456789", "border": "#56789a", "text": "#6789ab",
            "number": "#789abc", "translation": "#89abcd", "showSelectedBar": false,
        })
    );
    assert_eq!(json["candidate"]["light"]["text"], "#123");
    assert_eq!(json["candidate"]["light"]["showSelectedBar"], true);
    assert!(json["candidate"]["light"]["accent"].is_null());
    assert_eq!(
        scan_manifest(&manifest("sample")).packages[0].candidate,
        CandidateColors::default()
    );
}

#[test]
fn candidate_color_fields_enforce_types_and_utf8_byte_limits() {
    for theme in ["dark", "light"] {
        for key in [
            "accent",
            "selected",
            "hover",
            "surface",
            "border",
            "text",
            "number",
            "translation",
        ] {
            for value in [
                "false".to_owned(),
                "7".to_owned(),
                "[]".to_owned(),
                "{}".to_owned(),
                format!("'{}'", "a".repeat(81)),
                format!("'{}'", "色".repeat(27)),
            ] {
                let body = format!(
                    "{}\n[candidate.{theme}]\n{key} = {value}\n",
                    manifest("sample")
                );
                let catalog = scan_manifest(&body);
                assert!(catalog.packages.is_empty(), "accepted {theme}.{key}");
                assert_eq!(catalog.issues.len(), 1);
            }
            for value in [String::new(), "a".repeat(80)] {
                let body = format!(
                    "{}\n[candidate.{theme}]\n{key} = '{value}'\n",
                    manifest("sample")
                );
                assert_eq!(scan_manifest(&body).packages.len(), 1);
            }
        }
    }
}

#[test]
fn candidate_tables_and_selected_bar_reject_wrong_types() {
    for suffix in [
        "[candidate]\ndark = false",
        "[candidate]\nlight = []",
        "[candidate.dark]\nshow_selected_bar = 'false'",
        "[candidate.light]\nshow_selected_bar = 1",
    ] {
        let catalog = scan_manifest(&format!("{}\n{suffix}\n", manifest("sample")));
        assert!(catalog.packages.is_empty());
        assert_eq!(catalog.issues[0].reason, "invalid candidate colors");
    }
    let body = format!("candidate = false\n{}", manifest("sample"));
    assert!(scan_manifest(&body).packages.is_empty());
}

#[test]
fn preserves_capabilities_dimensions_and_relative_resources_for_hosts() {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir_all(skin.join("images")).unwrap();
    fs::write(skin.join("toolbar.css"), "/* fixture */").unwrap();
    fs::write(skin.join("images/preview.svg"), "<svg/>").unwrap();
    let body = format!(
        "toolbar_stylesheet = 'toolbar.css'\npreview = 'images/preview.svg'\n{}",
        manifest("sample")
    )
    .replace("['vertical']", "['vertical', 'horizontal']")
    .replace("min_width_dip = 10", "min_width_dip = 320.5")
    .replace("top_inset_dip = 0", "top_inset_dip = 24.5")
    .replace("width_dip = 0", "width_dip = 180");
    fs::write(skin.join("skin.toml"), body).unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.layouts, ["vertical", "horizontal"]);
    assert_eq!(package.themes, ["light"]);
    assert!(package.supports("vertical", "light"));
    assert!(package.supports("horizontal", "light"));
    assert!(!package.supports("vertical", "dark"));
    assert!(!package.supports("unknown", "light"));
    let json = serde_json::to_value(&catalog).unwrap();
    let package = &json["packages"][0];
    assert_eq!(package["minWidthDip"], 320.5);
    assert_eq!(package["decorationTopDip"], 24.5);
    assert_eq!(package["decorationWidthDip"], 180.0);
    assert_eq!(package["toolbarStylesheet"], "toolbar.css");
    assert_eq!(package["preview"], "images/preview.svg");
    assert_eq!(
        package["layouts"],
        serde_json::json!(["vertical", "horizontal"])
    );
}

#[test]
fn compatibility_does_not_inherit_unlisted_base_modes() {
    let catalog = scan_manifest(&manifest("sample"));
    let package = &catalog.packages[0];
    assert_eq!(package.base, super::super::theme::GlobalTheme::Night);
    assert!(package.supports("vertical", "light"));
    assert!(!package.supports("horizontal", "light"));
    assert!(!package.supports("vertical", "dark"));
    assert_eq!(package.toolbar_stylesheet, None);
    assert_eq!(package.preview, None);
}

#[test]
fn rejects_duplicate_supported_layouts_and_themes() {
    for (from, to) in [
        ("['vertical']", "['vertical', 'vertical']"),
        ("['light']", "['light', 'light']"),
    ] {
        let result = scan_manifest(&manifest("sample").replace(from, to));
        assert!(result.packages.is_empty());
        assert_eq!(result.issues[0].reason, "invalid supports");
    }
}

#[test]
fn rejects_wrong_numeric_types_and_nonfinite_or_out_of_range_dimensions() {
    for field in ["min_width_dip = 10", "top_inset_dip = 0", "width_dip = 0"] {
        let key = field.split(" = ").next().unwrap();
        for value in ["'10'", "false", "[]", "{}", "nan", "inf", "-1", "1001"] {
            let result =
                scan_manifest(&manifest("sample").replace(field, &format!("{key} = {value}")));
            assert!(result.packages.is_empty(), "accepted {key}={value}");
            assert_eq!(result.issues.len(), 1);
        }
    }
    assert_eq!(scan_manifest(&manifest("sample")).packages.len(), 1);
    let defaults = manifest("sample")
        .replace("min_width_dip = 10\n", "")
        .replace("top_inset_dip = 0\n", "")
        .replace("width_dip = 0\n", "");
    assert_eq!(scan_manifest(&defaults).packages.len(), 1);
}

#[test]
fn rejects_theme_ids_as_external_skin_folders() {
    let root = tempdir().unwrap();
    for id in [
        "system", "shuishan", "light", "paper", "night", "ink", "custom",
    ] {
        let skin = root.path().join(id);
        fs::create_dir(&skin).unwrap();
        fs::write(skin.join("skin.toml"), manifest(id)).unwrap();
    }
    let catalog = scan(root.path());
    assert!(catalog.packages.is_empty());
    assert_eq!(catalog.issues.len(), 7);
}

#[test]
fn toolbar_stylesheet_must_be_a_single_regular_css_file() {
    for (resource, directory) in [
        ("toolbar.css", false),
        ("toolbar.css", true),
        ("nested/toolbar.css", false),
        (".css", false),
    ] {
        let root = tempdir().unwrap();
        let skin = root.path().join("sample");
        fs::create_dir_all(skin.join("nested")).unwrap();
        let path = skin.join(resource);
        if directory {
            fs::create_dir(path).unwrap();
        } else {
            fs::write(path, "/* fixture */").unwrap();
        }
        fs::write(
            skin.join("skin.toml"),
            format!("toolbar_stylesheet = '{resource}'\n{}", manifest("sample")),
        )
        .unwrap();
        let result = scan(root.path());
        assert_eq!(
            result.packages.len(),
            usize::from(resource == "toolbar.css" && !directory)
        );
    }
}
#[test]
fn scans_valid_and_rejects_unsafe_manifests() {
    let dir = tempdir().unwrap();
    let skin = dir.path().join("sample_skin");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), "schema_version = 1\nid = 'sample_skin'\nname = 'Sample'\nversion = '1.0'\nbase = 'system'\nauthor = 'Test'\ndescription = 'Demo'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\n").unwrap();
    let catalog = scan(dir.path());
    assert_eq!(
        catalog.packages,
        vec![SkinSummary {
            schema_version: 1,
            id: "sample_skin".into(),
            name: "Sample".into(),
            version: "1.0".into(),
            // A system base draws over the platform tokens.
            base: super::super::theme::GlobalTheme::System,
            author: Some("Test".into()),
            description: Some("Demo".into()),
            layouts: vec!["vertical".into()],
            themes: vec!["light".into()],
            min_width_dip: 0.0,
            canvas: false,
            corner_radius_dip: None,
            border_width_dip: None,
            item_corner_radius_dip: None,
            shadow: None,
            backdrop: None,
            font_family: None,
            page_arrows: None,
            fit_content: None,
            expand_panel: None,
            expand_rows: None,
            geometry: SkinGeometry::default(),
            horizontal: SkinGeometry::default(),
            vertical: SkinGeometry::default(),
            decoration_top_dip: 0.0,
            decoration_width_dip: 0.0,
            decoration_image: None,
            decoration_align: DecorationAlign::Right,
            decoration_motion: DecorationMotion::None,
            decoration_motion_period_ms: DEFAULT_DECORATION_MOTION_PERIOD_MS,
            decoration_frames: 0,
            background: None,
            toolbar: SkinToolbar::default(),
            toolbar_icon_corner_radius_dip: None,
            border_image: None,
            content_insets_dip: None,
            line_gap_dip: None,
            outer_insets_dip: None,
            decorations: Vec::new(),
            toolbar_stylesheet: None,
            preview: None,
            candidate: CandidateColors::default(),
            panel_candidate: None,
            animation: SkinAnimation::disabled(),
            license: None,
            notices: Vec::new(),
        }],
        "{catalog:?}"
    );
}

#[test]
fn schema_v2_uses_bounded_lively_animation_presets() {
    let root = tempdir().unwrap();
    let skin = root.path().join("motion");
    fs::create_dir(&skin).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 2\nid = 'motion'\nname = 'Motion'\nversion = '2.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[animation]\nenter = 'spring'\nexit = 'fade'\nselection = 'glow'\nduration_ms = 240\nstagger_ms = 20\n",
    )
    .unwrap();
    let package = load(root.path(), "motion").unwrap();
    assert_eq!(package.schema_version, 2);
    assert_eq!(package.animation.enter, SkinAnimationPreset::Spring);
    assert_eq!(package.animation.selection, SkinAnimationPreset::Glow);
    assert_eq!(package.animation.duration_ms, 240);
    assert_eq!(
        package.animation.declared,
        SkinAnimationDeclared {
            enter: true,
            exit: true,
            selection: true,
        }
    );
}

#[test]
fn schema_v2_animation_table_keeps_lively_defaults_when_fields_are_omitted() {
    let root = tempdir().unwrap();
    let skin = root.path().join("motion_defaults");
    fs::create_dir(&skin).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 2\nid = 'motion_defaults'\nname = 'Motion defaults'\nversion = '2.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[animation]\nduration_ms = 200\n",
    )
    .unwrap();
    let package = load(root.path(), "motion_defaults").unwrap();
    assert_eq!(package.animation.enter, SkinAnimationPreset::FadeScale);
    assert_eq!(package.animation.exit, SkinAnimationPreset::Fade);
    assert_eq!(package.animation.selection, SkinAnimationPreset::Spring);
    assert_eq!(package.animation.duration_ms, 200);
    assert_eq!(package.animation.stagger_ms, 14);
    // 补齐的默认预设不算清单写了的：macOS 只播放写了的预设。
    assert_eq!(package.animation.declared, SkinAnimationDeclared::NONE);
}

#[test]
fn schema_v2_animation_reports_which_presets_the_manifest_declares() {
    let root = tempdir().unwrap();
    for (id, animation) in [
        ("motion_implicit", ""),
        ("motion_enter", "[animation]\nenter = 'slide_up'\n"),
    ] {
        let skin = root.path().join(id);
        fs::create_dir(&skin).unwrap();
        fs::write(
            skin.join("skin.toml"),
            format!("schema_version = 2\nid = '{id}'\nname = 'Motion'\nversion = '2.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n{animation}"),
        )
        .unwrap();
    }
    let implicit = load(root.path(), "motion_implicit").unwrap();
    assert_eq!(implicit.animation, SkinAnimation::lively());
    assert_eq!(implicit.animation.declared, SkinAnimationDeclared::NONE);
    let enter = load(root.path(), "motion_enter").unwrap();
    assert_eq!(enter.animation.enter, SkinAnimationPreset::SlideUp);
    assert_eq!(enter.animation.selection, SkinAnimationPreset::Spring);
    assert_eq!(
        enter.animation.declared,
        SkinAnimationDeclared {
            enter: true,
            exit: false,
            selection: false,
        }
    );
    // 宿主读的是序列化后的 JSON，标记必须随它一起出去。
    assert_eq!(
        serde_json::to_value(&enter.animation).unwrap()["declared"],
        serde_json::json!({"enter": true, "exit": false, "selection": false})
    );
}

#[test]
fn schema_v2_non_canvas_keeps_animation_and_ignores_canvas_fields() {
    // msime 的 v2（`[animation]`、装饰图待机动效、APNG 帧）对任何基底都有效；只有 msime-windows 的画布专用键需要 `base = "canvas"`，其他基底上不校验、不生效，并记一条提示。
    for base in ["system", "night"] {
        let root = tempdir().unwrap();
        let skin = root.path().join("lively");
        fs::create_dir(&skin).unwrap();
        fs::write(skin.join("whale.png"), encoded_apng(6, 4, 3)).unwrap();
        fs::write(
            skin.join("skin.toml"),
            format!(
                "schema_version = 2
id = 'lively'
name = 'Lively'
version = '1.0'
base = '{base}'
[supports]
layouts = ['horizontal']
themes = ['light', 'dark']
[candidate_window]
line_gap_dip = 'not-a-number'
[candidate_window.decoration]
image = 'whale.png'
top_inset_dip = 20
width_dip = 40
motion = 'sway'
[candidate_window.border_image]
source = 'missing.png'
slice_dip = ['not-a-number']
[[candidate_window.decorations]]
image = 'missing.png'
[animation]
enter = 'spring'
exit = 'fade'
selection = 'glow'
"
            ),
        )
        .unwrap();

        let catalog = scan(root.path());
        assert!(catalog.issues.is_empty(), "{base}: {catalog:?}");
        let package = &catalog.packages[0];
        assert_eq!(package.schema_version, 2, "{base}");
        assert!(!package.canvas);
        assert_eq!(package.animation.enter, SkinAnimationPreset::Spring);
        assert_eq!(package.animation.exit, SkinAnimationPreset::Fade);
        assert_eq!(package.animation.selection, SkinAnimationPreset::Glow);
        assert_eq!(package.decoration_motion, DecorationMotion::Sway);
        assert_eq!(package.decoration_frames, 3);
        assert!(package.border_image.is_none());
        assert!(package.line_gap_dip.is_none());
        assert!(package.decorations.is_empty());
        assert_eq!(package.notices.len(), 1, "{base}: {:?}", package.notices);
        for key in ["border_image", "line_gap_dip", "decorations"] {
            assert!(package.notices[0].contains(key), "{}", package.notices[0]);
        }
        let json = serde_json::to_value(package).unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["animation"]["enter"], "spring");
        assert_eq!(json["notices"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn schema_v2_non_canvas_still_validates_animation_and_motion() {
    let lively = |extra: &str| {
        format!("schema_version = 2\nid = 'sample'\nname = 'Sample'\nversion = '1.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\ntop_inset_dip = 20\nwidth_dip = 40\n{extra}")
    };
    for (extra, reason) in [
        ("motion = 'spin'\n", "invalid decoration motion"),
        (
            "motion_period_ms = 1199\n",
            "invalid decoration motion_period_ms",
        ),
        (
            "[animation]\nenter = 'not-a-preset'\n",
            "invalid animation enter",
        ),
        (
            "[animation]\nduration_ms = 'x'\n",
            "invalid animation duration_ms",
        ),
    ] {
        let catalog = scan_manifest(&lively(extra));
        assert!(catalog.packages.is_empty(), "{extra}");
        assert_eq!(catalog.issues[0].reason, reason, "{extra}");
    }
    // 版本 1 照旧不能写这些键。
    let catalog = scan_manifest(
        &lively("motion = 'float'\n").replace("schema_version = 2", "schema_version = 1"),
    );
    assert_eq!(
        catalog.issues[0].reason,
        "decoration motion requires schema_version 2"
    );
}

#[test]
fn schema_v2_without_canvas_only_keys_has_no_notices() {
    let catalog = scan_manifest("schema_version = 2\nid = 'sample'\nname = 'Sample'\nversion = '1.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[animation]\nenter = 'fade'\n");
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert!(catalog.packages[0].notices.is_empty());
    let json = serde_json::to_value(&catalog.packages[0]).unwrap();
    assert!(json.get("notices").is_none());
}

#[test]
fn canvas_packages_keep_panel_colours_and_readable_host_fallback() {
    let root = tempdir().unwrap();
    // 未绘制九宫格的宿主使用可读的 canvas 配色；有绘制能力的宿主从 panelCandidate 取得贴图原配色。
    let paneled = root.path().join("paneled");
    fs::create_dir(&paneled).unwrap();
    fs::write(paneled.join("panel.png"), encoded_png(12, 12)).unwrap();
    fs::write(
        paneled.join("skin.toml"),
        "schema_version = 2\nid = 'paneled'\nname = 'Paneled'\nversion = '1'\nbase = 'canvas'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\n[candidate_window.border_image]\nsource = 'panel.png'\nslice_dip = [4]\n[candidate.light]\ntext = '#f5ecff'\nnumber = '#f5ecff'\naccent = '#ff00aa'\n",
    )
    .unwrap();
    let package = load(root.path(), "paneled").unwrap();
    assert!(package.canvas);
    assert!(package.border_image.is_some());
    assert_eq!(package.base, super::super::theme::GlobalTheme::System);
    let light = &package.candidate.light;
    assert_eq!(light.surface.as_deref(), Some("#FFFFFF"));
    assert_eq!(light.text.as_deref(), Some("#1A1A1A"));
    assert_eq!(light.number.as_deref(), Some("#1A1A1A99"));
    assert_eq!(light.accent.as_deref(), Some("#ff00aa"));
    assert_eq!(light.translation.as_deref(), Some("#6940A5"));
    // 可画面板的宿主仍能拿到贴图对应的原色；回退和贴图配色各自完整序列化。
    let panel = package.panel_candidate.as_ref().unwrap();
    assert_eq!(panel.light.text.as_deref(), Some("#f5ecff"));
    assert_eq!(panel.light.number.as_deref(), Some("#f5ecff"));
    assert_eq!(panel.light.surface.as_deref(), Some("#FFFFFF"));
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["candidate"]["light"]["text"], "#1A1A1A");
    assert_eq!(json["panelCandidate"]["light"]["text"], "#f5ecff");
    let published = host_candidate_catalog(&scan(root.path()), root.path(), &["paneled"]);
    assert_eq!(published["directory"], root.path().to_str().unwrap());
    let entry = published["packages"][0].clone();
    assert_eq!(entry["candidate"]["light"]["text"], "#1A1A1A");
    assert!(super::super::theme::ThemePackage::from_host_catalog_entry(entry).is_ok());
    assert!(
        host_candidate_catalog(&scan(root.path()), Path::new("skins"), &[])
            .get("directory")
            .is_none()
    );
    assert_eq!(package.candidate.dark.surface.as_deref(), Some("#202020"));
    assert_eq!(package.candidate.dark.text.as_deref(), Some("#F2F2F2"));
    assert_eq!(package.corner_radius_dip, None);
    assert_eq!(package.toolbar.corner_radius_dip, Some(8.0));
    assert_eq!(package.toolbar.light.handle.as_deref(), Some("#6B69D6"));
    assert_eq!(package.notices, vec![CANVAS_RENDERER_NOTICE.to_owned()]);

    // 没有九宫格的 canvas 包只补没写的颜色，自己写的照用，也没有提示。
    let plain = root.path().join("plain");
    fs::create_dir(&plain).unwrap();
    fs::write(
        plain.join("skin.toml"),
        "schema_version = 2\nid = 'plain'\nname = 'Plain'\nversion = '1'\nbase = 'canvas'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate.light]\ntext = '#301020'\n",
    )
    .unwrap();
    let package = load(root.path(), "plain").unwrap();
    assert_eq!(package.candidate.light.text.as_deref(), Some("#301020"));
    assert_eq!(package.candidate.light.surface.as_deref(), Some("#FFFFFF"));
    assert!(package.notices.is_empty());
    assert!(package.panel_candidate.is_none());
}

#[test]
fn wechat_scroll_packages_inherit_the_detail_slots_and_menu_of_the_look() {
    let body = format!(
        "{}[candidate.dark]\naccent = '#123456'\n[candidate.light]\ntranslation = '#010203'\n[candidate.light.menu]\ntext = '#040506'\n",
        manifest("sample").replace("base = 'night'", "base = 'wechat_scroll'")
    );
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    let dark = &package.candidate.dark;
    assert_eq!(dark.accent.as_deref(), Some("#123456"));
    assert_eq!(dark.translation.as_deref(), Some("#529CCA"));
    assert_eq!(dark.preedit_text.as_deref(), Some("#D0D0D0"));
    assert_eq!(dark.selected_text.as_deref(), Some("#FFFFFF"));
    assert_eq!(dark.selected_number.as_deref(), Some("#FFFFFFCC"));
    assert_eq!(
        dark.menu.as_ref().unwrap().background.as_deref(),
        Some("#1F1F1F")
    );
    let light = &package.candidate.light;
    assert_eq!(light.translation.as_deref(), Some("#010203"));
    let menu = light.menu.as_ref().unwrap();
    assert_eq!(menu.text.as_deref(), Some("#040506"));
    assert_eq!(menu.hover.as_deref(), Some("#EEEEEE"));
}

#[test]
fn empty_author_and_description_mean_not_provided() {
    let catalog = scan_manifest(&format!(
        "author = ''\ndescription = ''\n{}",
        manifest("sample")
    ));
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages[0].author, None);
    assert_eq!(catalog.packages[0].description, None);
    let catalog = scan_manifest(&format!(
        "author = '{}'\n{}",
        "a".repeat(121),
        manifest("sample")
    ));
    assert_eq!(catalog.issues[0].reason, "author has invalid length");
}

#[test]
fn toolbar_colours_accept_the_four_digit_hex_form() {
    let catalog = scan_manifest(&format!(
        "{}[toolbar.light]\nbackground = '#e8a4'\n",
        manifest("sample")
    ));
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(
        catalog.packages[0].toolbar.light.background.as_deref(),
        Some("#EE88AA44")
    );
}

#[test]
fn schema_v1_cannot_enable_animation() {
    let root = tempdir().unwrap();
    let skin = root.path().join("legacy");
    fs::create_dir(&skin).unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 1\nid = 'legacy'\nname = 'Legacy'\nversion = '1.0'\nbase = 'system'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[animation]\nenter = 'fade'\n",
    )
    .unwrap();
    assert_eq!(
        load(root.path(), "legacy"),
        Err("animation requires schema_version 2".into())
    );
}

#[test]
fn reports_invalid_ids_and_oversized_manifests_without_loading_them() {
    let dir = tempdir().unwrap();
    let invalid = dir.path().join("Bad");
    fs::create_dir(&invalid).unwrap();
    fs::write(invalid.join("skin.toml"), "schema_version = 1").unwrap();
    let huge = dir.path().join("huge");
    fs::create_dir(&huge).unwrap();
    fs::write(huge.join("skin.toml"), vec![b'x'; 65_537]).unwrap();
    let catalog = scan(dir.path());
    assert!(catalog.packages.is_empty());
    assert!(catalog
        .issues
        .iter()
        .any(|issue| issue.folder == "huge" && issue.reason.contains("too large")));
}

#[test]
fn rejects_manifest_id_mismatch_and_missing_base() {
    let dir = tempdir().unwrap();
    for (folder, body) in [
        (
            "mismatch",
            "schema_version = 1\nid = 'other'\nname = 'X'\nversion = '1'\nbase = 'night'",
        ),
        (
            "baseless",
            "schema_version = 1\nid = 'baseless'\nname = 'X'\nversion = '1'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\n",
        ),
    ] {
        let path = dir.path().join(folder);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("skin.toml"), body).unwrap();
    }
    let catalog = scan(dir.path());
    assert!(catalog.packages.is_empty());
    assert_eq!(catalog.issues.len(), 2);
    assert!(catalog
        .issues
        .iter()
        .any(|issue| issue.folder == "baseless" && issue.reason == "base must be a string"));
}

#[test]
fn base_names_system_or_a_builtin_theme() {
    use super::super::theme::GlobalTheme;
    for (base, expected) in [
        ("system", GlobalTheme::System),
        ("shuishan", GlobalTheme::Shuishan),
        ("light", GlobalTheme::Light),
        ("paper", GlobalTheme::Paper),
        ("night", GlobalTheme::Night),
        ("ink", GlobalTheme::Ink),
    ] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.issues.is_empty(), "{base}: {catalog:?}");
        assert_eq!(catalog.packages[0].base, expected, "{base}");
    }
    // The msime-windows packages' `fluent` is the native Fluent look, which is `system` here.
    let catalog = scan_manifest(&manifest("sample").replace("base = 'night'", "base = 'fluent'"));
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(catalog.packages[0].base, GlobalTheme::System);
    assert_eq!(
        serde_json::to_value(&catalog.packages[0]).unwrap()["base"],
        "system"
    );
    // msime-windows 的其他内置外观同样画在 `system` 之上。
    for base in [
        "wechat",
        "wechat_scroll",
        "graphite",
        "willow_green",
        "autumn_osmanthus",
        "microsoft",
    ] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.issues.is_empty(), "{base}: {catalog:?}");
        assert_eq!(catalog.packages[0].base, GlobalTheme::System, "{base}");
    }
    // `custom` 和只在部分宿主上提供的 `native` 都不能当底，其他已停用或拼错的 ID 一律拒绝，不当作 system。
    for base in ["custom", "native", "Fluent", "Night", "Wechat", "default"] {
        let catalog = scan_manifest(
            &manifest("sample").replace("base = 'night'", &format!("base = '{base}'")),
        );
        assert!(catalog.packages.is_empty(), "{base}");
        assert_eq!(
            catalog.issues[0].reason, "base must be system or a built-in theme",
            "{base}"
        );
    }
}

#[test]
fn host_catalog_keeps_only_what_candidate_hosts_draw() {
    let body = "schema_version = 1\nid = 'sample'\nname = '樱花'\nversion = '1.0'\nbase = 'paper'\ntoolbar_stylesheet = 'toolbar.css'\n[supports]\nlayouts = ['vertical']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#FFF0F5'\ntext = '#123'\nhover = '#abcdef'\nborder = 'rgba(0,0,0,0.1)'\nshow_selected_bar = true\n[candidate.dark]\nselected = '#ff69b4'\nborder = '#ff000080'\n";
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("skin.toml"), body).unwrap();
    fs::write(skin.join("toolbar.css"), ".bar {}").unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    // The title is the manifest name; paths and stylesheets stay out, and every colour `theme::resolve` reads is published in the one form it emits.
    let published = host_candidate_catalog(&catalog, root.path(), &["absent"]);
    assert_eq!(
        published,
        serde_json::json!({"packages": [{
            "id": "sample",
            "title": "樱花",
            "base": "paper",
            "layouts": ["vertical"],
            "candidate": {
                "light": {
                    "surface": "#FFF0F5",
                    "text": "#112233",
                    "hover": "#ABCDEF",
                    "border": "#0000001A",
                    "show_selected_bar": true,
                },
                "dark": {"selected": "#FF69B4", "border": "#FF000080"},
            },
        }]})
    );
}

#[test]
fn a_published_entry_resolves_exactly_as_the_scanned_package() {
    use super::super::theme::{resolve, GlobalTheme, ThemePackage};
    use crate::preferences::CandidateLayout;
    // Every slot, each in a form the Windows presenter accepts but the contract does not emit.
    let palette = "surface = '#fef'\nborder = 'rgba(0, 0, 0, 0.25)'\ntext = 'rgb(17, 34, 51)'\nnumber = '#abc8'\naccent = '#ff69b4'\nselected = 'transparent'\nhover = 'rgba(255,255,255,0.5)'\ntranslation = 'rgba(1, 2, 3, .5)'\nshow_selected_bar = false\n";
    let bases = ["system", "paper", "night"];
    for base in bases {
        let body = format!(
            "schema_version = 1\nid = 'sample'\nname = 'S'\nversion = '1'\nbase = '{base}'\n[supports]\nlayouts = ['vertical']\nthemes = ['light', 'dark']\n[candidate_window]\n[candidate_window.decoration]\n[candidate.light]\n{palette}[candidate.dark]\n{}",
            palette.replace("#fef", "#102030")
        );
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("sample")).unwrap();
        fs::write(root.path().join("sample/skin.toml"), body).unwrap();
        let catalog = scan(root.path());
        assert!(catalog.issues.is_empty(), "{catalog:?}");
        let scanned = ThemePackage::from(&catalog.packages[0]);
        let entry =
            host_candidate_catalog(&catalog, root.path(), &["sample"])["packages"][0].clone();
        let published = ThemePackage::from_host_catalog_entry(entry).unwrap();
        let custom = crate::preferences::CustomTheme {
            candidate_skin: Some("sample".into()),
            ..Default::default()
        };
        for dark in [false, true] {
            // The package declares only the vertical layout, so the horizontal one checks that both sources carry the same gate.
            for layout in [CandidateLayout::Vertical, CandidateLayout::Horizontal] {
                let from_root = resolve(GlobalTheme::Custom, &custom, dark, layout, Some(&scanned));
                // 包只在声明的排列、且只在它 `base` 的明暗下绘制：paper 底只画在浅色，night 底只画在深色。
                assert_eq!(
                    from_root.candidate_skin.is_some(),
                    layout == CandidateLayout::Vertical && scanned.draws_in(dark),
                    "{base} {dark} {layout:?}"
                );
                assert_eq!(
                    resolve(GlobalTheme::Custom, &custom, dark, layout, Some(&published)),
                    from_root,
                    "{base} {dark} {layout:?}"
                );
            }
        }
    }
}

#[test]
fn host_catalog_carries_a_decoration_only_for_decorated_packages() {
    let root = tempdir().unwrap();
    let decorated = root.path().join("sakura");
    fs::create_dir_all(decorated.join("images")).unwrap();
    fs::write(decorated.join("images/ears.png"), b"png").unwrap();
    let body = format!("preview = 'images/ears.png'\n{}", manifest("sakura"))
        .replace("top_inset_dip = 0", "top_inset_dip = 24.5")
        .replace("width_dip = 0", "width_dip = 180");
    fs::write(decorated.join("skin.toml"), body).unwrap();
    // A preview without a decoration is only a picture for the settings page.
    let plain = root.path().join("plain");
    fs::create_dir(&plain).unwrap();
    fs::write(plain.join("preview.png"), b"png").unwrap();
    let body = format!("preview = 'preview.png'\n{}", manifest("plain"));
    fs::write(plain.join("skin.toml"), body).unwrap();
    // A decoration whose preview is not an image has nothing a host could draw.
    let styled = root.path().join("styled");
    fs::create_dir(&styled).unwrap();
    fs::write(styled.join("look.css"), ".x {}").unwrap();
    let body = format!("preview = 'look.css'\n{}", manifest("styled"))
        .replace("top_inset_dip = 0", "top_inset_dip = 10")
        .replace("width_dip = 0", "width_dip = 10");
    fs::write(styled.join("skin.toml"), body).unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let published = host_candidate_catalog(&catalog, root.path(), &[]);
    let package = |id: &str| {
        published["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["id"] == id)
            .unwrap()
            .clone()
    };
    let expected = decorated.join("images/ears.png");
    assert_eq!(
        package("sakura"),
        serde_json::json!({
            "id": "sakura",
            "title": "Sample",
            "base": "night",
            "layouts": ["vertical"],
            "candidate": {"light": {}},
            "decoration_top_dip": 24.5,
            "decoration_width_dip": 180.0,
            "decoration_image": expected.to_str().unwrap(),
        })
    );
    assert_eq!(
        package("plain"),
        serde_json::json!({"id": "plain", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}})
    );
    assert_eq!(
        package("styled"),
        serde_json::json!({"id": "styled", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}})
    );
    // The host opens the path as published, so a relative root publishes none.
    assert!(
        host_candidate_catalog(&catalog, Path::new("skins"), &[])["packages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|package| package.get("decoration_image").is_none())
    );
}

#[test]
fn host_catalog_drops_undeclared_themes_and_caps_its_size() {
    // manifest() declares only the light theme, as Windows would not draw this skin in dark.
    let body = format!(
        "{}\n[candidate.light]\nborder = 'transparent'\n[candidate.dark]\nsurface = '#000000'\n",
        manifest("sample")
    );
    let catalog = scan_manifest(&body);
    assert_eq!(
        host_candidate_catalog(&catalog, Path::new("/skins"), &[])["packages"][0]["candidate"],
        serde_json::json!({"light": {"border": "#00000000"}})
    );
    // A package without colours for a declared theme still lists, with an empty palette for that theme so the entry still says the package may be drawn in it.
    let bare = host_candidate_catalog(
        &scan_manifest(&manifest("sample")),
        Path::new("/skins"),
        &["sample"],
    );
    assert_eq!(
        bare,
        serde_json::json!({"packages": [{"id": "sample", "title": "Sample", "base": "night", "layouts": ["vertical"], "candidate": {"light": {}}}]})
    );

    let root = tempdir().unwrap();
    for index in 0..HOST_CATALOG_MAX_PACKAGES + 3 {
        let id = format!("skin{index:02}");
        fs::create_dir(root.path().join(&id)).unwrap();
        fs::write(root.path().join(&id).join("skin.toml"), manifest(&id)).unwrap();
    }
    let catalog = scan(root.path());
    assert_eq!(catalog.packages.len(), HOST_CATALOG_MAX_PACKAGES + 3);
    let ids = |published: serde_json::Value| {
        published["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|package| package["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let listed: Vec<_> = catalog
        .packages
        .iter()
        .map(|package| package.id.clone())
        .collect();
    assert_eq!(
        ids(host_candidate_catalog(&catalog, root.path(), &["absent"])),
        listed[..HOST_CATALOG_MAX_PACKAGES]
    );
    // A selected skin beyond the cap takes the last place instead of disappearing from the host's menu and colours.
    let beyond = listed[HOST_CATALOG_MAX_PACKAGES + 1].clone();
    let published = ids(host_candidate_catalog(&catalog, root.path(), &[&beyond]));
    assert_eq!(published.len(), HOST_CATALOG_MAX_PACKAGES);
    assert_eq!(
        published[..HOST_CATALOG_MAX_PACKAGES - 1],
        listed[..HOST_CATALOG_MAX_PACKAGES - 1]
    );
    assert_eq!(published.last(), Some(&beyond));
    // 浅色、深色两个槽位的皮肤都在上限之外时，两个都保留，挤掉上限内最后两个没选中的，各自保持目录顺序。
    let light = listed[HOST_CATALOG_MAX_PACKAGES + 2].clone();
    let published = ids(host_candidate_catalog(
        &catalog,
        root.path(),
        &[&light, &beyond],
    ));
    assert_eq!(published.len(), HOST_CATALOG_MAX_PACKAGES);
    assert_eq!(
        published[..HOST_CATALOG_MAX_PACKAGES - 2],
        listed[..HOST_CATALOG_MAX_PACKAGES - 2]
    );
    assert_eq!(
        published[HOST_CATALOG_MAX_PACKAGES - 2..],
        [beyond.clone(), light]
    );
    // 一个在上限内、一个在上限外：上限内的不动，只挤掉上限内最后一个没选中的。
    let inside = listed[HOST_CATALOG_MAX_PACKAGES - 1].clone();
    let published = ids(host_candidate_catalog(
        &catalog,
        root.path(),
        &[&inside, &beyond],
    ));
    assert_eq!(published.len(), HOST_CATALOG_MAX_PACKAGES);
    assert_eq!(
        published[..HOST_CATALOG_MAX_PACKAGES - 2],
        listed[..HOST_CATALOG_MAX_PACKAGES - 2]
    );
    assert_eq!(published[HOST_CATALOG_MAX_PACKAGES - 2..], [inside, beyond]);
}

#[test]
fn external_ids_are_the_folder_names_the_scan_lists() {
    for id in ["sample", "0day", "a.b_c-d"] {
        assert!(is_external_id(id), "{id}");
    }
    for id in [
        "",
        "Sample",
        ".hidden",
        "-dash",
        "a b",
        "system",
        "native",
        "night",
        "custom",
        "fluent",
        "wechat",
        "graphite",
        "willow_green",
        "autumn_osmanthus",
        "microsoft",
        "default",
        &"a".repeat(65),
    ] {
        assert!(!is_external_id(id), "{id}");
    }
}

/// `native` 成为全局主题 ID 之前是合法的皮肤文件夹名，偏好里保存着它的设置仍要能读；其他全局主题 ID 一直不能当皮肤选择。
#[test]
fn a_skin_selection_named_native_stays_valid() {
    assert!(is_selectable_id("native"));
    assert!(is_reserved("native"));
    for id in ["system", "night", "custom"] {
        assert!(!is_selectable_id(id), "{id}");
    }
}

/// msime-windows 的外观作为 `base` 时，包没写的颜色和圆角按该外观补齐，深浅各补各的；包自己写的、以及读得懂的颜色保持原样，读不懂的换成外观的颜色。
#[test]
fn a_windows_look_base_fills_what_the_package_leaves_out() {
    let body = manifest("sample").replace("base = 'night'", "base = 'wechat'")
        + "[candidate.dark]\naccent = '#123456'\ntext = 'not a colour'\n[toolbar.light]\nicon = '#abcdef'\n";
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.base, super::super::theme::GlobalTheme::System);
    let dark = &package.candidate.dark;
    assert_eq!(dark.accent.as_deref(), Some("#123456"));
    assert_eq!(dark.text.as_deref(), Some("#B7B7B7"));
    assert_eq!(dark.surface.as_deref(), Some("#151515"));
    assert_eq!(dark.hover.as_deref(), Some("#07C16052"));
    assert_eq!(dark.show_selected_bar, Some(false));
    // 翻译色跟随序号色，外观不补。
    assert_eq!(dark.translation, None);
    let light = &package.candidate.light;
    assert_eq!(light.surface.as_deref(), Some("#F7F7F7"));
    assert_eq!(light.selected.as_deref(), Some("#07C160"));
    assert_eq!(package.corner_radius_dip, Some(5.0));
    assert_eq!(package.toolbar.corner_radius_dip, Some(8.0));
    assert_eq!(package.toolbar.light.icon.as_deref(), Some("#ABCDEF"));
    assert_eq!(package.toolbar.light.handle.as_deref(), Some("#07C160"));
    assert_eq!(package.toolbar.dark.background.as_deref(), Some("#151515"));

    // 包自己写的圆角优先；`fluent` 就是原生配色，什么也不补。
    let own_radius = scan_manifest(
        &manifest("sample")
            .replace("base = 'night'", "base = 'graphite'")
            .replace(
                "min_width_dip = 10\n",
                "min_width_dip = 10\ncorner_radius_dip = 12\n",
            ),
    );
    assert_eq!(own_radius.packages[0].corner_radius_dip, Some(12.0));
    let fluent = scan_manifest(&manifest("sample").replace("base = 'night'", "base = 'fluent'"));
    assert_eq!(fluent.packages[0].candidate, CandidateColors::default());
    assert_eq!(fluent.packages[0].corner_radius_dip, None);
    assert_eq!(fluent.packages[0].toolbar, SkinToolbar::default());
}

/// The full-TOML manifest the native hosts must accept exactly as the settings page does (Windows parses skin.toml with toml++): literal strings, a multi-line array, an inline table, a digit separator, a unicode escape and a `#` inside a literal string.
const FULL_TOML_MANIFEST: &str = r#"schema_version = 1
id = 'full-toml'
name = "\u6768\u67f3 Full"
version = '1.0'
base = 'ink'
description = 'hash # inside a literal'
toolbar_stylesheet = 'toolbar.css'

[supports]
layouts = [
  'horizontal', # trailing comment
  'vertical',
]
themes = ['dark', 'light']

[candidate_window]
min_width_dip = 1_0
decoration = { top_inset_dip = 0, width_dip = 0 }

[candidate.light]
accent = '#c45c7a'
show_selected_bar = false
"#;

#[test]
fn load_package_accepts_full_toml_manifests() {
    let root = tempdir().unwrap();
    let skin = root.path().join("full-toml");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("skin.toml"), FULL_TOML_MANIFEST).unwrap();
    fs::write(skin.join("toolbar.css"), ".toolbar {}").unwrap();
    let package = load_package(root.path(), "full-toml").unwrap();
    assert_eq!(package.name, "杨柳 Full");
    assert_eq!(package.base, super::super::theme::GlobalTheme::Ink);
    assert_eq!(
        package.description.as_deref(),
        Some("hash # inside a literal")
    );
    assert_eq!(package.layouts, ["horizontal", "vertical"]);
    assert_eq!(package.min_width_dip, 10.0);
    assert_eq!(
        (package.decoration_top_dip, package.decoration_width_dip),
        (0.0, 0.0)
    );
    assert_eq!(package.toolbar_stylesheet.as_deref(), Some("toolbar.css"));
    assert_eq!(package.candidate.light.accent.as_deref(), Some("#c45c7a"));
    assert_eq!(package.candidate.light.show_selected_bar, Some(false));
    // One package resolves to what the full scan reports for it.
    assert_eq!(scan(root.path()).packages, vec![package]);
}

#[test]
fn load_package_rejects_theme_mismatched_missing_and_aliased_packages() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("ink")).unwrap();
    fs::write(root.path().join("ink/skin.toml"), manifest("ink")).unwrap();
    assert_eq!(
        load_package(root.path(), "ink"),
        Err("invalid skin id".into())
    );
    fs::create_dir_all(root.path().join("renamed")).unwrap();
    fs::write(root.path().join("renamed/skin.toml"), manifest("original")).unwrap();
    assert_eq!(
        load_package(root.path(), "renamed"),
        Err("manifest id does not match folder".into())
    );
    assert!(load_package(root.path(), "absent").is_err());
    assert!(load_package(root.path(), "../escape").is_err());
    resource_package(root.path());
    #[cfg(unix)]
    {
        // scan() skips a symlinked package directory, so resolving one by id must not draw it either.
        std::os::unix::fs::symlink(root.path().join("sample"), root.path().join("alias")).unwrap();
        assert!(load_package(root.path(), "alias").is_err());
    }
    assert!(load_package(root.path(), "sample").is_ok());
}

#[test]
#[cfg(unix)]
#[cfg_attr(
    target_os = "android",
    ignore = "Android 的 adb shell 域不允许建 FIFO（SELinux 拒绝 fifo_file create）"
)]
fn a_fifo_manifest_is_rejected_without_blocking() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("pipe")).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(root.path().join("pipe/skin.toml"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        load_package(root.path(), "pipe"),
        Err("skin.toml is not a regular file".into())
    );
    assert_eq!(scan(root.path()).issues[0].folder, "pipe");
}

#[test]
#[cfg(unix)]
fn a_symlinked_manifest_is_rejected_even_when_it_stays_inside_the_package() {
    let root = tempdir().unwrap();
    let skin = root.path().join("linked");
    fs::create_dir_all(&skin).unwrap();
    fs::write(skin.join("real.toml"), manifest("linked")).unwrap();
    std::os::unix::fs::symlink(skin.join("real.toml"), skin.join("skin.toml")).unwrap();
    assert_eq!(
        load_package(root.path(), "linked"),
        Err("skin.toml is not a regular file".into())
    );
    assert!(scan(root.path()).packages.is_empty());
}

#[test]
#[cfg(unix)]
fn a_symlinked_catalog_root_is_not_scanned() {
    use std::os::unix::fs::symlink;

    let state = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let package = resource_package(outside.path());
    fs::write(package.join("images/sample.png"), b"synthetic").unwrap();
    let root = state.path().join("skins");
    symlink(outside.path(), &root).unwrap();

    let catalog = scan(&root);
    assert!(catalog.packages.is_empty());
    assert!(catalog.issues.is_empty());
    assert!(load_package(&root, "sample").is_err());
    assert!(read_resource(&root, "sample", "images/sample.png").is_err());
}

/// The layout msime-skins (github.com/metasequoiaime/msime-skins) writes: a decoration with its own image and alignment, a background image, a corner radius, a toolbar palette per mode, a translation colour and licence metadata.
fn styled_package(root: &Path) -> std::path::PathBuf {
    let skin = root.join("bigfish");
    fs::create_dir_all(skin.join("assets")).unwrap();
    fs::write(skin.join("assets/character.png"), b"png").unwrap();
    fs::write(skin.join("assets/background.png"), b"png").unwrap();
    fs::write(
        skin.join("skin.toml"),
        "schema_version = 1\nid = 'bigfish'\nname = '蓝色大肥鱼'\nversion = '1.0.0'\nbase = 'system'\n\
         [supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['dark', 'light']\n\
         [candidate_window]\nmin_width_dip = 176\ncorner_radius_dip = 12\n\
         [candidate_window.decoration]\nimage = 'assets/character.png'\ntop_inset_dip = 104\nwidth_dip = 96\nalign = 'left'\n\
         [candidate_window.background]\nimage = 'assets/background.png'\nfit = 'contain'\nopacity = 0.35\n\
         [candidate.dark]\nnumber = '#E0C07A'\ntranslation = '#9FB4E0'\n\
         [candidate.light]\ntranslation = 'rgba(90, 106, 150, .5)'\n\
         [toolbar]\ncorner_radius_dip = 8\n\
         [toolbar.dark]\nbackground = '#141B33'\nborder = 'rgba(91, 155, 255, 0.38)'\nhandle = '#5B9BFF'\ndivider = 'rgba(91, 155, 255, 0.28)'\nicon = '#E3EAFF'\nhover = 'not a colour'\n\
         [toolbar.light]\nbackground = '#f4f8ff'\n\
         [license]\ncode = 'MIT'\nassets = 'CC-BY-4.0'\nsource = 'synthetic'\n",
    )
    .unwrap();
    skin
}

#[test]
fn styled_manifests_load_every_drawn_key() {
    let root = tempdir().unwrap();
    styled_package(root.path());
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.corner_radius_dip, Some(12.0));
    assert_eq!(
        package.decoration_image.as_deref(),
        Some("assets/character.png")
    );
    assert_eq!(package.decoration_align, DecorationAlign::Left);
    assert_eq!(
        package.background,
        Some(SkinBackground {
            image: "assets/background.png".into(),
            fit: BackgroundFit::Contain,
            opacity: 0.35,
        })
    );
    // Toolbar colours arrive normalized; one a host could not read is left out.
    assert_eq!(
        package.toolbar,
        SkinToolbar {
            corner_radius_dip: Some(8.0),
            dark: ToolbarPalette {
                background: Some("#141B33".into()),
                border: Some("#5B9BFF61".into()),
                handle: Some("#5B9BFF".into()),
                divider: Some("#5B9BFF47".into()),
                icon: Some("#E3EAFF".into()),
                hover: None,
            },
            light: ToolbarPalette {
                background: Some("#F4F8FF".into()),
                ..Default::default()
            },
        }
    );
    assert_eq!(
        package.license,
        Some(SkinLicense {
            code: Some("MIT".into()),
            assets: Some("CC-BY-4.0".into()),
            source: Some("synthetic".into()),
        })
    );
    let json = serde_json::to_value(package).unwrap();
    assert_eq!(json["cornerRadiusDip"], 12.0);
    assert_eq!(json["decorationImage"], "assets/character.png");
    assert_eq!(json["decorationAlign"], "left");
    assert_eq!(
        json["background"],
        serde_json::json!({"image": "assets/background.png", "fit": "contain", "opacity": 0.35})
    );
    assert_eq!(json["toolbar"]["cornerRadiusDip"], 8.0);
    assert_eq!(json["toolbar"]["dark"]["handle"], "#5B9BFF");
    assert_eq!(json["candidate"]["dark"]["translation"], "#9FB4E0");
}

#[test]
fn defaults_leave_new_keys_to_the_host() {
    // No decoration table at all, as msime-skins writes a package without one.
    let body = manifest("sample").replace(
        "[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n",
        "",
    );
    let catalog = scan_manifest(&body);
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    let package = &catalog.packages[0];
    assert_eq!(package.decoration_top_dip, 0.0);
    assert_eq!(package.corner_radius_dip, None);
    assert_eq!(package.decoration_image, None);
    assert_eq!(package.decoration_align, DecorationAlign::Right);
    assert_eq!(package.decoration_motion, DecorationMotion::None);
    assert_eq!(package.decoration_frames, 0);
    assert_eq!(package.background, None);
    assert_eq!(package.toolbar, SkinToolbar::default());
    assert_eq!(package.license, None);
    let json = serde_json::to_value(package).unwrap();
    assert!(json["cornerRadiusDip"].is_null());
    assert_eq!(json["decorationAlign"], "right");
    // A background without fit or opacity covers the card at full strength.
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("bg.webp"), b"webp").unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!(
            "{}[candidate_window.background]\nimage = 'bg.webp'\n",
            manifest("sample")
        ),
    )
    .unwrap();
    let catalog = scan(root.path());
    assert!(catalog.issues.is_empty(), "{catalog:?}");
    assert_eq!(
        catalog.packages[0].background,
        Some(SkinBackground {
            image: "bg.webp".into(),
            fit: BackgroundFit::Cover,
            opacity: 1.0,
        })
    );
}

#[test]
fn decoration_image_prefers_its_own_key_over_the_preview() {
    let root = tempdir().unwrap();
    let skin = root.path().join("sample");
    fs::create_dir(&skin).unwrap();
    fs::write(skin.join("preview.png"), b"png").unwrap();
    fs::write(skin.join("ears.png"), b"png").unwrap();
    let decorated = |extra: &str| {
        format!("preview = 'preview.png'\n{}", manifest("sample"))
            .replace("top_inset_dip = 0", &format!("{extra}top_inset_dip = 20"))
            .replace("width_dip = 0", "width_dip = 40")
    };
    fs::write(skin.join("skin.toml"), decorated("image = 'ears.png'\n")).unwrap();
    assert_eq!(
        scan(root.path()).packages[0].decoration_image.as_deref(),
        Some("ears.png")
    );
    fs::write(skin.join("skin.toml"), decorated("")).unwrap();
    assert_eq!(
        scan(root.path()).packages[0].decoration_image.as_deref(),
        Some("preview.png")
    );
    // An undecorated package draws no decoration, whatever its preview.
    fs::write(
        skin.join("skin.toml"),
        format!("preview = 'preview.png'\n{}", manifest("sample")),
    )
    .unwrap();
    assert_eq!(scan(root.path()).packages[0].decoration_image, None);
}

#[test]
fn styled_keys_out_of_bounds_reject_the_package() {
    let cases = [
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = 33",
            "invalid corner_radius_dip",
        ),
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = -1",
            "invalid corner_radius_dip",
        ),
        (
            "corner_radius_dip = 12",
            "corner_radius_dip = '12'",
            "invalid corner_radius_dip",
        ),
        ("align = 'left'", "align = 'top'", "invalid decoration"),
        ("align = 'left'", "align = 3", "align must be a string"),
        (
            "image = 'assets/character.png'",
            "image = 'assets/missing.png'",
            "invalid decoration",
        ),
        (
            "image = 'assets/character.png'",
            "image = '../outside.png'",
            "invalid decoration",
        ),
        (
            "image = 'assets/character.png'",
            "image = 'skin.toml'",
            "invalid decoration",
        ),
        ("fit = 'contain'", "fit = 'tile'", "invalid background"),
        ("opacity = 0.35", "opacity = 1.5", "invalid background"),
        ("opacity = 0.35", "opacity = -0.1", "invalid background"),
        (
            "image = 'assets/background.png'",
            "image = 'assets/background.txt'",
            "invalid background",
        ),
        (
            "image = 'assets/background.png'",
            "image = '/etc/passwd.png'",
            "invalid background",
        ),
        (
            "[toolbar]\ncorner_radius_dip = 8",
            "[toolbar]\ncorner_radius_dip = 40",
            "invalid toolbar",
        ),
        (
            "background = '#141B33'",
            "background = 5",
            "invalid toolbar",
        ),
        (
            "background = '#141B33'",
            &format!("background = '{}'", "a".repeat(81)),
            "toolbar color exceeds 80 bytes",
        ),
        (
            "translation = '#9FB4E0'",
            "translation = 7",
            "invalid candidate colors",
        ),
        ("code = 'MIT'", "code = ''", "code has invalid length"),
    ];
    for (from, to, reason) in cases {
        let root = tempdir().unwrap();
        let skin = styled_package(root.path());
        let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
        assert_eq!(body.matches(from).count(), 1, "{from}");
        fs::write(skin.join("skin.toml"), body.replacen(from, to, 1)).unwrap();
        let catalog = scan(root.path());
        assert!(catalog.packages.is_empty(), "accepted {to}");
        assert_eq!(catalog.issues[0].reason, reason, "{to}");
    }
    // A decoration image needs a band to be drawn in.
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        body.replace("top_inset_dip = 104\nwidth_dip = 96\n", ""),
    )
    .unwrap();
    assert_eq!(scan(root.path()).issues[0].reason, "invalid decoration");
    // Tables where a table belongs.
    for (from, to) in [
        (
            "[candidate_window.background]\n",
            "[candidate_window]\nbackground = 1\n[x]\n",
        ),
        ("[toolbar.light]\n", "[toolbar]\nlight = 1\n[y]\n"),
    ] {
        let root = tempdir().unwrap();
        let skin = styled_package(root.path());
        let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
        fs::write(skin.join("skin.toml"), body.replacen(from, to, 1)).unwrap();
        assert!(scan(root.path()).packages.is_empty(), "accepted {to}");
    }
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let body = fs::read_to_string(skin.join("skin.toml")).unwrap();
    fs::write(
        skin.join("skin.toml"),
        format!("license = 1\n{}", body.replace("[license]\n", "[z]\n")),
    )
    .unwrap();
    assert_eq!(scan(root.path()).issues[0].reason, "invalid license");
}

#[test]
fn host_catalog_publishes_what_linux_draws_of_a_styled_package() {
    let root = tempdir().unwrap();
    let skin = styled_package(root.path());
    let catalog = scan(root.path());
    let published = host_candidate_catalog(&catalog, root.path(), &["bigfish"]);
    // The background and toolbar stay out: neither Linux host can draw them.
    assert_eq!(
        published,
        serde_json::json!({"packages": [{
            "id": "bigfish",
            "title": "蓝色大肥鱼",
            "base": "system",
            "layouts": ["horizontal", "vertical"],
            "candidate": {
                "dark": {"number": "#E0C07A", "translation": "#9FB4E0"},
                "light": {"translation": "#5A6A9680"},
            },
            "decoration_top_dip": 104.0,
            "decoration_width_dip": 96.0,
            "decoration_image": skin.join("assets/character.png").to_str().unwrap(),
            "decoration_align": "left",
            "corner_radius_dip": 12.0,
        }]})
    );
}

/// `client_dialect.json` 是皮肤清单规则的共享用例表，各个实现都按它校验：这里的 `load`、msime-cloud 的 Go 移植（`internal/skins/client.go`）与种子脚本，以及 msime-windows 的 `CandidateSkinCatalog::Load`。它们各自保存一份副本，由各自仓库的同步脚本按本文件刷新，所以改规则先改这里。
///
/// 每个用例按 `_comment` 说明的顺序生成清单与旁边的文件（每个文件一个字节），接受的用例核对解析出的 `base`，拒绝的用例核对拒绝原因。
#[test]
fn shared_dialect_cases_match_the_loader() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("client_dialect.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 150, "用例表丢了用例：{}", cases.len());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let template = case["template"].as_str().unwrap();
        let (mut id, mut body, mut files) = if template == "raw" {
            let body = case["manifest"].as_str().unwrap().to_owned();
            ("sample".to_owned(), body, Vec::new())
        } else {
            let source = &fixture["templates"][template];
            (
                source["id"].as_str().unwrap().to_owned(),
                source["manifest"].as_str().unwrap().to_owned(),
                source["files"].as_array().unwrap().clone(),
            )
        };
        if let Some(own) = case["id"].as_str() {
            id = own.to_owned();
        }
        body = body.replace("{id}", &id);
        for edit in case["replace"].as_array().into_iter().flatten() {
            let from = edit[0].as_str().unwrap();
            assert!(body.contains(from), "{name}：清单里没有 {from:?}");
            body = body.replacen(from, edit[1].as_str().unwrap(), 1);
        }
        body = format!(
            "{}{body}{}",
            case["prepend"].as_str().unwrap_or(""),
            case["append"].as_str().unwrap_or("")
        );
        if let Some(pad_to) = case["pad_to"].as_u64() {
            body.push('#');
            body.push_str(&"x".repeat(pad_to as usize - body.len()));
        }
        if let Some(own) = case["files"].as_array() {
            files = own.clone();
        }

        let root = tempdir().unwrap();
        let skin = root.path().join(&id);
        fs::create_dir_all(&skin).unwrap();
        for file in &files {
            let path = skin.join(file.as_str().unwrap());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        fs::write(skin.join("skin.toml"), &body).unwrap();
        match (load(root.path(), &id), case["reason"].as_str()) {
            (Ok(package), None) => {
                if let Some(base) = case["base"].as_str() {
                    assert_eq!(package.base.id(), base, "{name}");
                }
            }
            (Err(reason), Some(expected)) => assert_eq!(reason, expected, "{name}"),
            (Ok(_), Some(expected)) => panic!("{name}：接受了，应以 {expected:?} 拒绝"),
            (Err(reason), None) => panic!("{name}：以 {reason:?} 拒绝，应接受"),
        }
    }
}
