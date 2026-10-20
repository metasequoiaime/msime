use super::*;
use crate::skin::catalog::{self, SkinLength};

fn package(base: &str, window: &str) -> SkinSummary {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("synthetic");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join("skin.toml"),
        format!(
        "schema_version = 2\nid = 'synthetic'\nname = '合成皮肤'\nversion = '1'\nbase = '{base}'\n\
         [supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n\
         [candidate_window]\n{window}"
    ),
    )
    .unwrap();
    catalog::load(root.path(), "synthetic").unwrap()
}

fn dip(value: f64) -> Option<SkinLength> {
    Some(SkinLength { value, em: false })
}

#[test]
fn web_factory_layout_copy_matches_shared_resolution() {
    let layouts: serde_json::Map<String, serde_json::Value> = factories()
        .iter()
        .map(|(base, _)| {
            let resolved = package(base, "").resolved_layout.unwrap();
            (base.to_string(), serde_json::to_value(resolved).unwrap())
        })
        .collect();
    let layouts = serde_json::Value::Object(layouts);
    // 网页构建只需 Node；镜像由共享解析器生成并逐字段比对，不另维护 TOML 解析器。
    if std::env::var_os("MSIME_WRITE_FACTORY_LAYOUTS").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/web-engine/tools/factory-layouts.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&layouts).unwrap() + "\n",
        )
        .unwrap();
    }
    // 设备单测不具备构建机源码目录，正常校验使用编译时内嵌的镜像。
    let copy: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/web-engine/tools/factory-layouts.json"
    )))
    .unwrap();
    assert_eq!(
        copy, layouts,
        "网页出厂版式镜像已过时，请设置 MSIME_WRITE_FACTORY_LAYOUTS=1 重跑本测试"
    );
}

#[test]
fn all_factory_bases_reach_renderers_without_rewriting_declarations() {
    // 值来自固定来源的八份出厂清单，覆盖不同的圆角、开关和排列默认。
    for (base, radius, border, item_radius, padding) in [
        ("fluent", 6.0, 1.5, 4.0, 5.0),
        ("wechat", 5.0, 1.0, 4.0, 2.0),
        ("wechat_scroll", 10.0, 1.0, 6.0, 5.0),
        ("graphite", 3.0, 1.0, 2.0, 5.0),
        ("willow_green", 9.0, 0.0, 0.0, 0.0),
        ("autumn_osmanthus", 10.0, 0.0, 6.0, 5.0),
        ("microsoft", 8.0, 1.0, 4.0, 0.3),
        ("canvas", 0.0, 0.0, 4.0, 5.0),
    ] {
        let package = package(base, "");
        assert!(
            package.geometry.is_empty()
                && package.horizontal.is_empty()
                && package.vertical.is_empty()
        );
        assert_eq!(package.item_corner_radius_dip, None);
        assert_eq!(package.page_arrows, None);
        let resolved = package.resolved_layout.as_ref().unwrap();
        assert_eq!(resolved.corner_radius_dip, radius, "{base}");
        assert_eq!(resolved.border_width_dip, border, "{base}");
        assert_eq!(resolved.item_corner_radius_dip, item_radius, "{base}");
        assert_eq!(resolved.page_arrows, base == "microsoft");
        assert_eq!(resolved.expand_panel, base == "wechat_scroll");
        assert!(!resolved.fit_content);
        assert_eq!(resolved.expand_rows, 4);
        assert_eq!(
            resolved.geometry.padding,
            Some(SkinLength {
                value: padding,
                em: base == "microsoft"
            })
        );
        for geometry in [&resolved.horizontal, &resolved.vertical] {
            assert_eq!(
                geometry.padding,
                Some(SkinLength {
                    value: padding,
                    em: base == "microsoft"
                })
            );
            assert_eq!(
                geometry.number_scale,
                Some(if base == "microsoft" { 1.0 } else { 0.8 })
            );
            assert_eq!(geometry.selected_bar_width, dip(3.0));
        }
        assert_eq!(
            resolved.horizontal.item_min_width.is_some(),
            base == "microsoft"
        );
        assert!(resolved.vertical.item_min_width.is_none());
        let json = serde_json::to_value(&package).unwrap();
        assert!(!json.as_object().unwrap().contains_key("geometry"));
        assert_eq!(json["resolvedLayout"]["cornerRadiusDip"], radius);
        assert_eq!(
            json["resolvedLayout"]["horizontal"]["padding"]["value"],
            padding
        );
        if base == "canvas" {
            assert_eq!(resolved.shadow.as_deref(), Some("none"));
            assert_eq!(resolved.horizontal.item_corners_follow_card, Some(false));
        } else if base == "wechat_scroll" {
            assert_eq!(resolved.shadow.as_deref(), Some("soft"));
        }
    }
}

#[test]
fn factory_direction_defaults_are_distinct_and_em_is_not_premultiplied() {
    let microsoft = package("microsoft", "").resolved_layout.unwrap();
    assert_eq!(
        microsoft.horizontal.item_gap,
        Some(SkinLength {
            value: 0.15,
            em: true
        })
    );
    assert_eq!(
        microsoft.horizontal.item_min_width,
        Some(SkinLength {
            value: 3.5,
            em: true
        })
    );
    assert_eq!(microsoft.vertical.item_gap, dip(2.0));
    assert_eq!(microsoft.vertical.item_min_width, None);
    assert_eq!(
        microsoft.horizontal.page_arrow_style.as_deref(),
        Some("triangle")
    );
    let willow = package("willow_green", "").resolved_layout.unwrap();
    assert_eq!(willow.horizontal.item_extra_height, dip(10.0));
    assert_eq!(willow.vertical.item_extra_height, dip(8.0));
    assert_eq!(willow.horizontal.number_inset, dip(11.0));
    assert_eq!(willow.vertical.number_inset, dip(14.0));
    assert_eq!(willow.horizontal.preedit_margin_dip.unwrap().right, 5.0);
    assert_eq!(willow.vertical.preedit_margin_dip.unwrap().right, 7.0);
    let fluent = package("fluent", "").resolved_layout.unwrap();
    assert_eq!(fluent.horizontal.item_corners_follow_card, Some(true));
    assert_eq!(fluent.vertical.item_corners_follow_card, None);
}

#[test]
fn package_common_overrides_factory_direction_and_explicit_zero_and_false_survive() {
    let package = package("willow_green", "\
        corner_radius_dip = 0\nborder_width_dip = 0\nitem_corner_radius_dip = 0\n\
        shadow = 'none'\npage_arrows = false\nfit_content = false\nexpand_panel = false\nexpand_rows = 2\n\
        padding = 0\nitem_gap = 0\nitem_extra_height = 0\nnumber_inset = 0\nitem_padding_right = 0\n\
        item_min_width = 0\nnumber_gap = 0\nnumber_scale = 0\nselected_bar_width = 0\n\
        selected_bar_height_em = 0\nselected_bar_inside = false\nitem_corners_follow_card = false\n\
        justify_rows = false\ntrim_hidden_preedit_gap = false\npage_arrow_style = 'triangle'\n\
        preedit_margin_dip = [0, 0, 0, 0]\n");
    let resolved = package.resolved_layout.as_ref().unwrap();
    // 整张表比较，防止漏合并任何字段；公共声明也必须盖过 willow 的两张出厂排列表。
    assert_eq!(resolved.horizontal, package.geometry);
    assert_eq!(resolved.vertical, package.geometry);
    assert_eq!(resolved.geometry, package.geometry);
    assert_eq!(resolved.corner_radius_dip, 0.0);
    assert_eq!(resolved.item_corner_radius_dip, 0.0);
    assert!(!resolved.page_arrows && !resolved.fit_content && !resolved.expand_panel);
    assert_eq!(resolved.expand_rows, 2);
    let microsoft = package_for_override("microsoft");
    assert!(!microsoft.page_arrows);
    let scroll = package_for_override("wechat_scroll");
    assert!(!scroll.expand_panel);
}

fn package_for_override(base: &str) -> ResolvedSkinLayout {
    package(base, "page_arrows = false\nexpand_panel = false\n")
        .resolved_layout
        .unwrap()
}

#[test]
fn package_direction_wins_last_and_does_not_leak() {
    let package = package(
        "microsoft",
        "\
        item_gap = 13\nnumber_scale = 0.5\n\
        [candidate_window.horizontal]\nitem_gap = '0.7em'\nnumber_scale = 0\n",
    );
    let resolved = package.resolved_layout.unwrap();
    assert_eq!(
        resolved.horizontal.item_gap,
        Some(SkinLength {
            value: 0.7,
            em: true
        })
    );
    assert_eq!(resolved.vertical.item_gap, dip(13.0));
    assert_eq!(resolved.horizontal.number_scale, Some(0.0));
    assert_eq!(resolved.vertical.number_scale, Some(0.5));
}

#[test]
fn linux_host_catalog_publishes_the_resolved_layout() {
    let package = package("microsoft", "");
    let catalog = catalog::SkinCatalog {
        packages: vec![package],
        issues: Vec::new(),
    };
    let root = tempfile::tempdir().unwrap();
    let published = catalog::host_candidate_catalog(&catalog, root.path(), &[]);
    let entry = &published["packages"][0];
    assert_eq!(entry["resolvedLayout"]["geometry"]["padding"]["value"], 0.3);
    assert_eq!(
        entry["resolvedLayout"]["horizontal"]["itemMinWidth"]["value"],
        3.5
    );
    assert_eq!(entry["resolvedLayout"]["pageArrows"], true);
}

#[test]
fn native_and_existing_global_themes_keep_host_defaults() {
    for base in ["system", "shuishan", "light", "paper", "night", "ink"] {
        let package = package(base, "padding = 11\n");
        assert!(package.resolved_layout.is_none());
        let json = serde_json::to_value(package).unwrap();
        assert!(!json.as_object().unwrap().contains_key("resolvedLayout"));
        assert_eq!(json["geometry"]["padding"]["value"], 11.0);
    }
}
