//! `tests/fixtures/candidate-skins` 下的验证皮肤：每个包演示一组 msime-windows 兼容的皮肤键，提交在仓库里，Windows 的截图工具渲染的也是这一批包。这里用真实的扫描器读它们，逐个核对每个包要演示的那几项解析结果，并记下社区打包和 MCP 创建对它们各自接受与否。
//!
//! 图片由同目录的 `generate.py` 生成；改了清单或图片之后，这里的断言要一起改。

use super::*;
use crate::skin::candidate_community::{self, CandidateSkinPackage, CandidateSkinVisibility};
use crate::skin::theme::normalized_color;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use tempfile::tempdir;

// 设备上没有构建机源码路径，测试程序随身携带同一批合成素材。
fn gallery() -> tempfile::TempDir {
    let root = tempdir().unwrap();
    macro_rules! fixtures {
        ($($path:literal),+ $(,)?) => {
            $(
                let path = root.path().join($path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/candidate-skins/", $path))).unwrap();
            )+
        };
    }
    fixtures!(
        "verify-arrows-chevron/preview.png",
        "verify-arrows-chevron/skin.toml",
        "verify-arrows-triangle/preview.png",
        "verify-arrows-triangle/skin.toml",
        "verify-backdrop-mica/preview.png",
        "verify-backdrop-mica/skin.toml",
        "verify-canvas-frame/assets/frame.png",
        "verify-canvas-frame/assets/leaf.png",
        "verify-canvas-frame/assets/ribbon.png",
        "verify-canvas-frame/assets/seal.png",
        "verify-canvas-frame/assets/stars.gif",
        "verify-canvas-frame/preview.png",
        "verify-canvas-frame/skin.toml",
        "verify-canvas-keys-ignored/preview.png",
        "verify-canvas-keys-ignored/skin.toml",
        "verify-canvas/assets/cloud.png",
        "verify-canvas/assets/moon.png",
        "verify-canvas/assets/panel-dark.png",
        "verify-canvas/assets/panel-light.png",
        "verify-canvas/assets/twinkle.png",
        "verify-canvas/preview.png",
        "verify-canvas/skin.toml",
        "verify-color-syntax/preview.png",
        "verify-color-syntax/skin.toml",
        "verify-expand-panel/preview.png",
        "verify-expand-panel/skin.toml",
        "verify-frame-none/preview.png",
        "verify-frame-none/skin.toml",
        "verify-frame-strong/preview.png",
        "verify-frame-strong/skin.toml",
        "verify-layout-em/preview.png",
        "verify-layout-em/skin.toml",
        "verify-v2-motion-system/assets/koi.png",
        "verify-v2-motion-system/preview.png",
        "verify-v2-motion-system/skin.toml",
    );
    root
}

/// 目录里的全部验证皮肤，按名字排序。新加一个包时要在这里登记，并给它写断言。
const GALLERY: [&str; 12] = [
    "verify-arrows-chevron",
    "verify-arrows-triangle",
    "verify-backdrop-mica",
    "verify-canvas",
    "verify-canvas-frame",
    "verify-canvas-keys-ignored",
    "verify-color-syntax",
    "verify-expand-panel",
    "verify-frame-none",
    "verify-frame-strong",
    "verify-layout-em",
    "verify-v2-motion-system",
];

fn package(id: &str) -> SkinSummary {
    load_package(gallery().path(), id).unwrap_or_else(|reason| panic!("{id}: {reason}"))
}

fn em(value: f64) -> Option<SkinLength> {
    Some(SkinLength { value, em: true })
}

fn dip(value: f64) -> Option<SkinLength> {
    Some(SkinLength { value, em: false })
}

fn insets(top: f64, right: f64, bottom: f64, left: f64) -> Option<SkinInsets> {
    Some(SkinInsets {
        top,
        right,
        bottom,
        left,
    })
}

fn declared_all() -> SkinAnimationDeclared {
    SkinAnimationDeclared {
        enter: true,
        exit: true,
        selection: true,
    }
}

#[test]
fn every_gallery_skin_loads_without_an_issue() {
    let catalog = scan(gallery().path());
    assert!(catalog.issues.is_empty(), "{:?}", catalog.issues);
    let mut ids: Vec<&str> = catalog.packages.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, GALLERY);
    for package in &catalog.packages {
        let id = package.id.as_str();
        assert!(is_external_id(id), "{id}");
        assert_eq!(package.version, "1.0.0", "{id}");
        assert_eq!(package.preview.as_deref(), Some("preview.png"), "{id}");
        assert_eq!(package.themes, ["dark", "light"], "{id}");
        let layouts: &[&str] = if id == "verify-expand-panel" {
            &["horizontal"]
        } else {
            &["horizontal", "vertical"]
        };
        assert_eq!(package.layouts, layouts, "{id}");
        assert_eq!(
            package
                .license
                .as_ref()
                .and_then(|license| license.assets.as_deref()),
            Some("MIT"),
            "{id}"
        );
        let (schema_version, canvas) = match id {
            "verify-canvas" | "verify-canvas-frame" => (2, true),
            "verify-canvas-keys-ignored" | "verify-v2-motion-system" => (2, false),
            _ => (1, false),
        };
        assert_eq!(package.schema_version, schema_version, "{id}");
        assert_eq!(package.canvas, canvas, "{id}");
        // 只有 canvas 验证包带提示：两个是贴图需要宿主绘制能力，一个是非 canvas 基底上忽略的画布键。
        if !matches!(
            id,
            "verify-canvas" | "verify-canvas-frame" | "verify-canvas-keys-ignored"
        ) {
            assert!(package.notices.is_empty(), "{id}: {:?}", package.notices);
        }
        // v1 的包不能有动画；`windows_looks` 和 `canvas` 都折成 `system` 交给跨平台主题解析。
        if schema_version == 1 {
            assert_eq!(package.animation, SkinAnimation::disabled(), "{id}");
        }
        assert_eq!(
            package.base,
            super::super::theme::GlobalTheme::System,
            "{id}"
        );
    }
}

#[test]
fn layout_em_keeps_em_lengths_and_per_layout_overrides() {
    let package = package("verify-layout-em");
    assert_eq!(package.min_width_dip, 200.0);
    assert_eq!(package.corner_radius_dip, Some(10.0));
    assert_eq!(package.item_corner_radius_dip, Some(6.0));
    assert_eq!(
        package.geometry,
        SkinGeometry {
            padding: em(0.5),
            item_gap: em(0.6),
            item_extra_height: em(0.25),
            number_inset: em(0.3),
            item_padding_right: em(0.4),
            item_min_width: em(2.5),
            number_gap: em(0.2),
            number_scale: Some(0.8),
            selected_bar_width: em(0.2),
            selected_bar_height_em: Some(0.9),
            selected_bar_inside: Some(true),
            item_corners_follow_card: Some(true),
            justify_rows: None,
            trim_hidden_preedit_gap: Some(true),
            page_arrow_style: None,
            preedit_margin_dip: insets(2.0, 4.0, 6.0, 8.0),
        }
    );
    // 横排、竖排表只带自己写了的键，其余由宿主回到共享表；两个值、一个值的外边距按 CSS 展开。
    assert_eq!(
        package.horizontal,
        SkinGeometry {
            item_gap: dip(10.0),
            number_scale: Some(0.75),
            justify_rows: Some(true),
            preedit_margin_dip: insets(4.0, 6.0, 4.0, 6.0),
            ..SkinGeometry::default()
        }
    );
    assert_eq!(
        package.vertical,
        SkinGeometry {
            padding: em(0.75),
            item_extra_height: dip(6.0),
            selected_bar_width: dip(3.0),
            selected_bar_inside: Some(false),
            preedit_margin_dip: insets(6.0, 6.0, 6.0, 6.0),
            ..SkinGeometry::default()
        }
    );
    // 宿主读的 JSON 里 em 长度是 `{value, em: true}`，固定 DIP 是 `em: false`。
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(
        json["geometry"]["padding"],
        serde_json::json!({ "value": 0.5, "em": true })
    );
    assert_eq!(
        json["geometry"]["selectedBarWidth"],
        serde_json::json!({ "value": 0.2, "em": true })
    );
    assert_eq!(
        json["horizontal"]["itemGap"],
        serde_json::json!({ "value": 10.0, "em": false })
    );
    assert_eq!(json["vertical"]["selectedBarInside"], false);
    assert!(json["horizontal"].get("padding").is_none());
    assert_eq!(
        package.candidate.light.selected_bar.as_deref(),
        Some("#3E8E41")
    );
    assert_eq!(package.candidate.dark.show_selected_bar, Some(true));
}

#[test]
fn frame_strong_draws_a_thick_frame_and_strong_shadow() {
    let package = package("verify-frame-strong");
    assert_eq!(package.border_width_dip, Some(3.0));
    assert_eq!(package.corner_radius_dip, Some(14.0));
    assert_eq!(package.item_corner_radius_dip, Some(8.0));
    assert_eq!(package.shadow.as_deref(), Some("strong"));
    assert_eq!(package.backdrop, None);
    assert_eq!(package.fit_content, None);
    assert_eq!(package.toolbar.corner_radius_dip, Some(14.0));
    assert_eq!(package.candidate.light.border.as_deref(), Some("#B4233C"));
    assert_eq!(package.candidate.dark.border.as_deref(), Some("#E05A73"));
    assert_eq!(package.candidate.light.show_selected_bar, Some(false));
    assert_eq!(package.geometry, SkinGeometry::default());
}

#[test]
fn frame_none_has_no_frame_no_shadow_and_fits_its_content() {
    let package = package("verify-frame-none");
    assert_eq!(package.border_width_dip, Some(0.0));
    assert_eq!(package.shadow.as_deref(), Some("none"));
    assert_eq!(package.fit_content, Some(true));
    assert_eq!(package.min_width_dip, 0.0);
    assert_eq!(package.corner_radius_dip, Some(6.0));
    assert_eq!(package.item_corner_radius_dip, Some(4.0));
    assert_eq!(package.page_arrows, None);
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["fitContent"], true);
    assert_eq!(json["borderWidthDip"], 0.0);
}

#[test]
fn backdrop_mica_writes_both_material_parameters_per_mode() {
    let package = package("verify-backdrop-mica");
    assert_eq!(package.backdrop.as_deref(), Some("mica"));
    assert_eq!(package.shadow.as_deref(), Some("soft"));
    assert_eq!(package.corner_radius_dip, Some(8.0));
    assert_eq!(package.border_width_dip, Some(1.0));
    assert_eq!(package.border_image, None);
    // 两套材质参数都写出，底色半透明，留给画得出云母的宿主做色调。
    for (palette, surface, opacity, luminosity) in [
        (
            &package.candidate.light,
            "rgba(232, 244, 240, 0.6)",
            0.5,
            0.9,
        ),
        (&package.candidate.dark, "rgba(22, 38, 35, 0.7)", 0.8, 1.0),
    ] {
        assert_eq!(palette.surface.as_deref(), Some(surface));
        assert_eq!(palette.backdrop_opacity, Some(opacity));
        assert_eq!(palette.backdrop_luminosity, Some(luminosity));
    }
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["backdrop"], "mica");
    assert_eq!(json["candidate"]["dark"]["backdrop_opacity"], 0.8);
}

#[test]
fn arrows_chevron_on_the_microsoft_look_fills_what_it_leaves_out() {
    let package = package("verify-arrows-chevron");
    assert_eq!(package.page_arrows, Some(true));
    assert_eq!(
        package.geometry.page_arrow_style.as_deref(),
        Some("chevron")
    );
    // 没写的底色、外框、文字色和两处圆角按 `microsoft` 外观补齐，写了的蓝色照用。
    assert_eq!(package.corner_radius_dip, Some(8.0));
    assert_eq!(package.toolbar.corner_radius_dip, Some(8.0));
    let light = &package.candidate.light;
    assert_eq!(light.surface.as_deref(), Some("#F9F9F9"));
    assert_eq!(light.border.as_deref(), Some("#0000001A"));
    assert_eq!(light.text.as_deref(), Some("#1A1A1A"));
    assert_eq!(light.accent.as_deref(), Some("#0F6CBD"));
    assert_eq!(light.number.as_deref(), Some("#3B6E9E"));
    assert_eq!(light.show_selected_bar, Some(true));
    let dark = &package.candidate.dark;
    assert_eq!(dark.surface.as_deref(), Some("#2C2C2C"));
    assert_eq!(dark.text.as_deref(), Some("#FFFFFF"));
    assert_eq!(dark.selected.as_deref(), Some("#1F3A57"));
    assert_eq!(dark.selected_bar.as_deref(), Some("#62ABF5"));
    assert_eq!(package.toolbar.light.handle.as_deref(), Some("#E183D9"));
}

#[test]
fn arrows_triangle_colours_its_divider() {
    let package = package("verify-arrows-triangle");
    assert_eq!(package.page_arrows, Some(true));
    assert_eq!(
        package.geometry.page_arrow_style.as_deref(),
        Some("triangle")
    );
    assert_eq!(
        package.candidate.light.page_arrow_divider.as_deref(),
        Some("#E8C77A")
    );
    assert_eq!(
        package.candidate.dark.page_arrow_divider.as_deref(),
        Some("#6B5420")
    );
    assert_eq!(package.border_width_dip, Some(1.0));
    assert_eq!(package.shadow.as_deref(), Some("soft"));
    assert_eq!(package.corner_radius_dip, Some(8.0));
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["geometry"]["pageArrowStyle"], "triangle");
    assert_eq!(json["candidate"]["dark"]["page_arrow_divider"], "#6B5420");
}

#[test]
fn expand_panel_on_the_wechat_look_is_horizontal_only() {
    let package = package("verify-expand-panel");
    assert_eq!(package.expand_panel, Some(true));
    assert_eq!(package.expand_rows, Some(3));
    assert!(package.supports("horizontal", "light"));
    assert!(package.supports("horizontal", "dark"));
    assert!(!package.supports("vertical", "light"));
    // 圆角和竖条开关按 `wechat` 外观补，颜色是包自己的薄荷绿。
    assert_eq!(package.corner_radius_dip, Some(5.0));
    assert_eq!(package.candidate.light.show_selected_bar, Some(false));
    assert_eq!(package.candidate.light.selected.as_deref(), Some("#10A37F"));
    assert_eq!(
        package.candidate.light.selected_text.as_deref(),
        Some("#FFFFFF")
    );
    assert_eq!(package.candidate.dark.surface.as_deref(), Some("#0F1E1A"));
    assert_eq!(package.toolbar.light.handle.as_deref(), Some("#07C160"));
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["expandPanel"], true);
    assert_eq!(json["expandRows"], 3);
}

#[test]
fn canvas_keeps_its_panel_insets_widgets_and_motion() {
    let package = package("verify-canvas");
    assert!(package.canvas);
    assert_eq!(
        package.border_image,
        Some(SkinBorderImage {
            source: "assets/panel-dark.png".into(),
            light_source: Some("assets/panel-light.png".into()),
            slice_dip: vec![16.0],
            stretch_dip: None,
            width_dip: Some(vec![8.0]),
            fill: true,
        })
    );
    assert_eq!(package.content_insets_dip, insets(10.0, 14.0, 10.0, 14.0));
    assert_eq!(package.outer_insets_dip, insets(18.0, 18.0, 12.0, 10.0));
    assert_eq!(package.line_gap_dip, Some(5.0));
    let widget =
        |image: &str, anchor: &str, x: f64, y: f64, size: (f64, f64), z: i32| SkinFreeDecoration {
            image: image.into(),
            anchor: anchor.into(),
            offset_x_dip: x,
            offset_y_dip: y,
            width_dip: size.0,
            height_dip: size.1,
            z_index: z,
        };
    assert_eq!(
        package.decorations,
        [
            widget(
                "assets/moon.png",
                "top-right",
                18.0,
                -18.0,
                (40.0, 40.0),
                -1
            ),
            widget(
                "assets/twinkle.png",
                "top-left",
                -10.0,
                -12.0,
                (24.0, 24.0),
                3
            ),
            widget("assets/cloud.png", "bottom", 40.0, 10.0, (48.0, 20.0), 1),
        ]
    );
    assert_eq!(package.notices, [CANVAS_RENDERER_NOTICE]);
    assert_eq!(
        package.animation,
        SkinAnimation {
            enter: SkinAnimationPreset::FadeScale,
            exit: SkinAnimationPreset::Fade,
            selection: SkinAnimationPreset::Glow,
            duration_ms: 220,
            stagger_ms: 18,
            declared: declared_all(),
        }
    );
    // 画不出贴图的宿主拿 `canvas` 外观的可读配色，只留包的强调色和右键菜单；画得出的宿主从 panelCandidate 取原配色。
    let fallback = &package.candidate.light;
    assert_eq!(fallback.surface.as_deref(), Some("#FFFFFF"));
    assert_eq!(fallback.text.as_deref(), Some("#1A1A1A"));
    assert_eq!(fallback.accent.as_deref(), Some("#7C3AED"));
    assert_eq!(
        fallback.menu.as_ref().unwrap().background.as_deref(),
        Some("#FBFAFF")
    );
    assert_eq!(package.candidate.dark.surface.as_deref(), Some("#202020"));
    let panel = package.panel_candidate.as_ref().unwrap();
    assert_eq!(panel.light.text.as_deref(), Some("#2E1065"));
    assert_eq!(
        panel.light.number.as_deref(),
        Some("rgba(46, 16, 101, 0.55)")
    );
    assert_eq!(panel.light.surface.as_deref(), Some("#FFFFFF"));
    assert_eq!(panel.dark.selected_bar.as_deref(), Some("#FDE68A"));
    assert_eq!(package.toolbar.corner_radius_dip, Some(10.0));
    assert_eq!(package.toolbar.dark.border.as_deref(), Some("#A78BFA59"));
    // 没有装饰带，挂件不算装饰图。
    assert_eq!(package.decoration_image, None);
    assert_eq!(package.decoration_frames, 0);
}

/// 挂件 APNG 的每一帧都要能完整解码；静态挂件和面板是单帧 PNG。
#[test]
fn canvas_images_decode_frame_by_frame() {
    let gallery = gallery();
    let assets = gallery.path().join("verify-canvas/assets");
    for (file, frames) in [
        ("twinkle.png", 6),
        ("moon.png", 1),
        ("cloud.png", 1),
        ("panel-dark.png", 1),
        ("panel-light.png", 1),
    ] {
        let bytes = fs::read(assets.join(file)).unwrap();
        assert_eq!(png_frames(&bytes), Ok((frames, frames)), "{file}");
        let mut reader = png::Decoder::new(Cursor::new(&bytes[..]))
            .read_info()
            .unwrap();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        for _ in 0..frames {
            reader.next_frame(&mut buffer).unwrap();
        }
    }
}

/// `verify-canvas` 没用到的画布写法：一张深浅共用的面板，stretch_dip 盖过 slice_dip、两值简写，不写 width_dip，fill = false；不写内容内边距、外留白和行距；挂件用上 center、left、top、bottom-right 四种锚点和一张 GIF。
#[test]
fn canvas_frame_keeps_its_stretch_hollow_panel_and_gif_widget() {
    let package = package("verify-canvas-frame");
    assert!(package.canvas);
    assert_eq!(
        package.border_image,
        Some(SkinBorderImage {
            source: "assets/frame.png".into(),
            light_source: None,
            slice_dip: vec![20.0],
            stretch_dip: Some(vec![10.0, 14.0]),
            width_dip: None,
            fill: false,
        })
    );
    assert_eq!(package.content_insets_dip, None);
    assert_eq!(package.outer_insets_dip, None);
    assert_eq!(package.line_gap_dip, None);
    let anchors: Vec<(&str, &str, f64, f64, i32)> = package
        .decorations
        .iter()
        .map(|d| {
            (
                d.image.as_str(),
                d.anchor.as_str(),
                d.offset_x_dip,
                d.offset_y_dip,
                d.z_index,
            )
        })
        .collect();
    assert_eq!(
        anchors,
        [
            ("assets/seal.png", "center", 0.0, 0.0, 0),
            ("assets/stars.gif", "left", -16.0, 0.0, 2),
            ("assets/ribbon.png", "top", 0.0, -10.0, 5),
            ("assets/leaf.png", "bottom-right", 12.0, 12.0, 1),
        ]
    );
    assert_eq!(package.notices, [CANVAS_RENDERER_NOTICE]);
    // 面板画得出来时用包写的原配色，画不出来时用 `canvas` 外观的可读配色。
    let panel = package.panel_candidate.as_ref().unwrap();
    assert_eq!(panel.light.text.as_deref(), Some("#14532D"));
    assert_eq!(package.candidate.light.text.as_deref(), Some("#1A1A1A"));
}

/// 挂件 GIF 是 4 帧整幅画布、带透明色的动图，每一帧都能完整解码，帧数计入像素额度。
#[test]
fn canvas_frame_gif_decodes_every_frame() {
    use image::AnimationDecoder;
    let bytes = fs::read(
        gallery()
            .path()
            .join("verify-canvas-frame/assets/stars.gif"),
    )
    .unwrap();
    let frames = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes[..]))
        .unwrap()
        .into_frames()
        .collect_frames()
        .unwrap();
    assert_eq!(frames.len(), 4);
    for frame in &frames {
        let buffer = frame.buffer();
        assert_eq!(buffer.dimensions(), (48, 48));
        // 四角是透明的，正中是星星。
        assert_eq!(buffer.get_pixel(0, 0)[3], 0);
        assert_eq!(buffer.get_pixel(24, 24)[3], 255);
    }
    // 面板图中心是品红：fill = false 时它不该被画出来，按 slice_dip = 20 切的话四角会带上它。
    let frame = image::load_from_memory(
        &fs::read(
            gallery()
                .path()
                .join("verify-canvas-frame/assets/frame.png"),
        )
        .unwrap(),
    )
    .unwrap()
    .to_rgba8();
    assert_eq!(frame.dimensions(), (48, 40));
    assert_eq!(frame.get_pixel(24, 20).0, [0xFF, 0x00, 0xFF, 0xFF]);
    for (x, y) in [(13, 9), (34, 9), (13, 30), (34, 30)] {
        assert_ne!(
            frame.get_pixel(x, y).0,
            [0xFF, 0x00, 0xFF, 0xFF],
            "({x}, {y})"
        );
    }
}

#[test]
fn v2_motion_on_the_system_base_animates_and_floats_an_apng() {
    let package = package("verify-v2-motion-system");
    assert!(!package.canvas);
    assert_eq!(package.schema_version, 2);
    assert_eq!(
        package.animation,
        SkinAnimation {
            enter: SkinAnimationPreset::SlideUp,
            exit: SkinAnimationPreset::Fade,
            selection: SkinAnimationPreset::Spring,
            duration_ms: 200,
            stagger_ms: 12,
            declared: declared_all(),
        }
    );
    assert_eq!(package.decoration_image.as_deref(), Some("assets/koi.png"));
    assert_eq!(package.decoration_top_dip, 26.0);
    assert_eq!(package.decoration_width_dip, 52.0);
    assert_eq!(package.decoration_align, DecorationAlign::Left);
    assert_eq!(package.decoration_motion, DecorationMotion::Float);
    assert_eq!(package.decoration_motion_period_ms, 3600);
    assert_eq!(package.decoration_frames, 8);
    let json = serde_json::to_value(&package).unwrap();
    assert_eq!(json["schemaVersion"], 2);
    assert_eq!(json["animation"]["enter"], "slide_up");
    assert_eq!(json["animation"]["declared"]["selection"], true);
    assert_eq!(json["decorationMotion"], "float");
    assert_eq!(json["decorationFrames"], 8);
    assert!(json.get("canvas").is_none());
}

/// `verify-color-syntax` 的颜色：(键, 清单原文, 规范化后)。候选和菜单颜色按原样进 `SkinSummary`，宿主再规范化；工具栏颜色在扫描时就已规范化。
const LIGHT_COLORS: [(&str, &str, &str); 22] = [
    ("accent", "#7B4FD6", "#7B4FD6"),
    ("selected", "rgba(123, 79, 214, 0.16)", "#7B4FD629"),
    ("hover", "#7B4FD614", "#7B4FD614"),
    ("surface", "rgba(253, 251, 255, 0.92)", "#FDFBFFEB"),
    ("border", "#DCF", "#DDCCFF"),
    ("text", "rgb(46, 31, 77)", "#2E1F4D"),
    ("number", "#546A", "#554466AA"),
    ("translation", "#8A7BA8", "#8A7BA8"),
    ("translation_pos", "#9b87c4", "#9B87C4"),
    (
        "translation_separator",
        "rgba(138, 123, 168, 0.45)",
        "#8A7BA873",
    ),
    ("candidate_text", "#2E1F4D", "#2E1F4D"),
    ("preedit_text", "#63C", "#6633CC"),
    ("preedit_caret", "rgb(214, 79, 160)", "#D64FA0"),
    ("selected_text", "#4B2A9E", "#4B2A9E"),
    ("selected_number", "#4b2a9eb3", "#4B2A9EB3"),
    (
        "selected_translation",
        "rgba(75, 42, 158, 0.6)",
        "#4B2A9E99",
    ),
    ("selected_translation_pos", "#B49BE6", "#B49BE6"),
    ("selected_translation_separator", "#4B2A9E66", "#4B2A9E66"),
    ("selected_bar", "#D64FA0", "#D64FA0"),
    ("preedit_background", "transparent", "#00000000"),
    ("preedit_divider", "rgba(123, 79, 214, 0.25)", "#7B4FD640"),
    ("page_arrow_divider", "#E5DBF7", "#E5DBF7"),
];

const DARK_COLORS: [(&str, &str, &str); 22] = [
    ("accent", "#B79CFF", "#B79CFF"),
    ("selected", "rgba(183, 156, 255, 0.22)", "#B79CFF38"),
    ("hover", "#B79CFF1F", "#B79CFF1F"),
    ("surface", "rgba(28, 22, 41, 0.9)", "#1C1629E6"),
    ("border", "#435", "#443355"),
    ("text", "rgb(236, 230, 250)", "#ECE6FA"),
    ("number", "#CBEB", "#CCBBEEBB"),
    ("translation", "#A99CC8", "#A99CC8"),
    ("translation_pos", "#c7b5f0", "#C7B5F0"),
    (
        "translation_separator",
        "rgba(169, 156, 200, 0.45)",
        "#A99CC873",
    ),
    ("candidate_text", "#ECE6FA", "#ECE6FA"),
    ("preedit_text", "#C9F", "#CC99FF"),
    ("preedit_caret", "rgb(255, 133, 200)", "#FF85C8"),
    ("selected_text", "#FFFFFF", "#FFFFFF"),
    ("selected_number", "#FFFFFFB3", "#FFFFFFB3"),
    (
        "selected_translation",
        "rgba(255, 255, 255, 0.75)",
        "#FFFFFFBF",
    ),
    ("selected_translation_pos", "#E3D5FF", "#E3D5FF"),
    ("selected_translation_separator", "#FFFFFF66", "#FFFFFF66"),
    ("selected_bar", "#FF85C8", "#FF85C8"),
    (
        "preedit_background",
        "rgba(255, 255, 255, 0.04)",
        "#FFFFFF0A",
    ),
    ("preedit_divider", "#B79CFF40", "#B79CFF40"),
    ("page_arrow_divider", "#3A2F55", "#3A2F55"),
];

const LIGHT_MENU: [(&str, &str, &str); 4] = [
    ("background", "#FFF", "#FFFFFF"),
    ("border", "#DCF8", "#DDCCFF88"),
    ("text", "rgb(46, 31, 77)", "#2E1F4D"),
    ("hover", "rgba(123, 79, 214, 0.1)", "#7B4FD61A"),
];

const DARK_MENU: [(&str, &str, &str); 4] = [
    ("background", "#1C1629", "#1C1629"),
    ("border", "#B79CFF33", "#B79CFF33"),
    ("text", "#ECE6FA", "#ECE6FA"),
    ("hover", "rgba(183, 156, 255, 0.16)", "#B79CFF29"),
];

/// 颜色原文属于七种写法中的哪一种。
fn color_syntax(value: &str) -> &'static str {
    match value.strip_prefix('#').map(str::len) {
        Some(3) => "#RGB",
        Some(4) => "#RGBA",
        Some(6) => "#RRGGBB",
        Some(8) => "#RRGGBBAA",
        _ if value.starts_with("rgba(") => "rgba()",
        _ if value.starts_with("rgb(") => "rgb()",
        _ if value == "transparent" => "transparent",
        _ => panic!("unexpected colour syntax {value}"),
    }
}

#[test]
fn color_syntax_writes_every_colour_key_in_every_syntax() {
    let package = package("verify-color-syntax");
    assert_eq!(package.backdrop.as_deref(), Some("acrylic"));
    assert_eq!(package.border_width_dip, Some(1.0));
    let mut syntaxes = BTreeSet::new();
    for (palette, colors, menu_colors, opacity, luminosity) in [
        (
            &package.candidate.light,
            &LIGHT_COLORS,
            &LIGHT_MENU,
            0.72,
            0.85,
        ),
        (&package.candidate.dark, &DARK_COLORS, &DARK_MENU, 0.6, 0.7),
    ] {
        let json = serde_json::to_value(palette).unwrap();
        // 每一个候选颜色键都写了：22 个颜色加两个材质参数，正好是 catalog 接受的全部键。
        for key in CANDIDATE_COLOR_KEYS {
            assert!(!json[key].is_null(), "{key}");
        }
        assert_eq!(colors.len() + 2, CANDIDATE_COLOR_KEYS.len());
        for (table, entries) in [(&json, &colors[..]), (&json["menu"], &menu_colors[..])] {
            for (key, raw, normalized) in entries {
                assert_eq!(table[key].as_str(), Some(*raw), "{key}");
                assert_eq!(normalized_color(raw).as_deref(), Some(*normalized), "{key}");
                syntaxes.insert(color_syntax(raw));
            }
        }
        let menu = serde_json::to_value(palette.menu.as_ref().unwrap()).unwrap();
        for key in MENU_COLOR_KEYS {
            assert!(!menu[key].is_null(), "menu {key}");
        }
        assert_eq!(palette.backdrop_opacity, Some(opacity));
        assert_eq!(palette.backdrop_luminosity, Some(luminosity));
        assert_eq!(palette.show_selected_bar, Some(true));
    }
    assert_eq!(
        syntaxes.into_iter().collect::<Vec<_>>(),
        [
            "#RGB",
            "#RGBA",
            "#RRGGBB",
            "#RRGGBBAA",
            "rgb()",
            "rgba()",
            "transparent"
        ]
    );
    // 工具栏颜色在扫描时规范化：`#RGB` 展开、小写转大写、`rgb()`/`rgba()` 换成十六进制、`transparent` 是全透明黑。
    let toolbar =
        |background: &str, border: &str, handle: &str, divider: &str, icon: &str, hover: &str| {
            ToolbarPalette {
                background: Some(background.into()),
                border: Some(border.into()),
                handle: Some(handle.into()),
                divider: Some(divider.into()),
                icon: Some(icon.into()),
                hover: Some(hover.into()),
            }
        };
    assert_eq!(
        package.toolbar.light,
        toolbar(
            "#FDFBFFF2",
            "#DDCCFF",
            "#7B4FD6",
            "#7B4FD633",
            "#2E1F4D",
            "#7B4FD61A"
        )
    );
    assert_eq!(
        package.toolbar.dark,
        toolbar(
            "#1C1629",
            "#00000000",
            "#BB99FF",
            "#B79CFF29",
            "#ECE6FA",
            "#B79CFF24"
        )
    );
    assert_eq!(package.toolbar.corner_radius_dip, Some(10.0));
    assert_eq!(package.toolbar_icon_corner_radius_dip, Some(6.0));
}

#[test]
fn canvas_keys_on_the_system_base_are_ignored_with_a_notice() {
    let package = package("verify-canvas-keys-ignored");
    assert!(!package.canvas);
    assert_eq!(package.schema_version, 2);
    assert_eq!(package.border_image, None);
    assert_eq!(package.content_insets_dip, None);
    assert_eq!(package.line_gap_dip, None);
    assert_eq!(package.outer_insets_dip, None);
    assert!(package.decorations.is_empty());
    assert_eq!(package.panel_candidate, None);
    assert_eq!(
        package.notices,
        ["candidate_window.border_image, candidate_window.content_insets_dip, candidate_window.line_gap_dip, candidate_window.outer_insets_dip, candidate_window.decorations only apply with base = \"canvas\" and schema_version = 2; ignored"]
    );
    assert_eq!(
        package.animation,
        SkinAnimation {
            enter: SkinAnimationPreset::Fade,
            exit: SkinAnimationPreset::SlideUp,
            selection: SkinAnimationPreset::Glow,
            duration_ms: 160,
            stagger_ms: 0,
            declared: declared_all(),
        }
    );
    // 这些键引用的图片故意不在包里：它们要是被校验了，包会被拒绝。
    assert!(!gallery()
        .path()
        .join("verify-canvas-keys-ignored/assets")
        .exists());
    assert_eq!(package.candidate.light.surface.as_deref(), Some("#F7FAFC"));
}

/// 包引用的全部图片：MCP 的 `create_candidate_skin` 要求传入的图片不多不少，正好是这些。
fn referenced_images(package: &SkinSummary) -> Vec<&str> {
    let mut paths: Vec<&str> = [
        package.preview.as_deref(),
        package.decoration_image.as_deref(),
        package.background.as_ref().map(|b| b.image.as_str()),
        package.border_image.as_ref().map(|b| b.source.as_str()),
        package
            .border_image
            .as_ref()
            .and_then(|b| b.light_source.as_deref()),
    ]
    .into_iter()
    .flatten()
    .chain(package.decorations.iter().map(|d| d.image.as_str()))
    .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

/// 验证图库的全部包都能社区打包和经 MCP 使用的原子安装路径创建，包括六张资源的 canvas。
#[test]
fn the_gallery_against_community_packing_and_mcp_creation() {
    let gallery = gallery();
    for id in GALLERY {
        let packed =
            candidate_community::pack_as(gallery.path(), id, CandidateSkinVisibility::Public);
        assert_eq!(
            packed.as_ref().map(|_| ()).map_err(|code| *code),
            Ok(()),
            "{id}: pack"
        );
        if let Ok(packed) = &packed {
            // 打包带上的正是包引用的那几张图，`canvas-keys-ignored` 里被忽略的键不引用任何图。
            let files: Vec<&str> = packed.files.keys().map(String::as_str).collect();
            assert_eq!(files, referenced_images(&package(id)), "{id}");
        }

        let summary = package(id);
        let files: BTreeMap<String, String> = referenced_images(&summary)
            .into_iter()
            .map(|path| {
                let bytes = fs::read(gallery.path().join(id).join(path)).unwrap();
                (path.to_owned(), BASE64.encode(bytes))
            })
            .collect();
        let manifest = fs::read_to_string(gallery.path().join(id).join("skin.toml")).unwrap();
        let root = tempdir().unwrap();
        let installed = candidate_community::install(
            root.path(),
            &CandidateSkinPackage {
                id: uuid::Uuid::nil(),
                package_id: id.to_owned(),
                manifest,
                files,
            },
            false,
        );
        assert_eq!(
            installed.as_ref().map(|_| ()).map_err(|code| *code),
            Ok(()),
            "{id}: create"
        );
        if installed.is_ok() {
            assert_eq!(load_package(root.path(), id).unwrap(), summary, "{id}");
        }
    }
}
