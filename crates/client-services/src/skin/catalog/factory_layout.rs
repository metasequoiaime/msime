//! 绘制布局按来源的四层顺序继承，不把出厂默认写回编辑器的原始声明。

use super::{read_geometry, SkinGeometry, SkinSummary};
use serde::Serialize;
use std::sync::OnceLock;
use toml::Value;

/// 所有支持布局的宿主共用的绘制输入。横竖排已分别合并，em 仍按实际候选字号换算。
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSkinLayout {
    pub corner_radius_dip: f64,
    pub border_width_dip: f64,
    pub item_corner_radius_dip: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    pub page_arrows: bool,
    pub fit_content: bool,
    pub expand_panel: bool,
    pub expand_rows: u8,
    pub geometry: SkinGeometry,
    pub horizontal: SkinGeometry,
    pub vertical: SkinGeometry,
}

fn factories() -> &'static [(&'static str, Value)] {
    static FACTORIES: OnceLock<Vec<(&str, Value)>> = OnceLock::new();
    FACTORIES.get_or_init(|| {
        macro_rules! manifests {
            ($($id:literal),+ $(,)?) => {
                vec![$(($id, toml::from_str(include_str!(concat!("factory-layouts/", $id, ".toml")))
                    .expect("内嵌出厂皮肤清单必须有效"))),+]
            };
        }
        manifests!(
            "fluent",
            "wechat",
            "wechat_scroll",
            "graphite",
            "willow_green",
            "autumn_osmanthus",
            "microsoft",
            "canvas"
        )
    })
}

impl SkinGeometry {
    fn overlay(&mut self, layer: &Self) {
        macro_rules! fields {
            ($($field:ident),+ $(,)?) => {
                $(if layer.$field.is_some() { self.$field.clone_from(&layer.$field); })+
            };
        }
        fields!(
            padding,
            item_gap,
            item_extra_height,
            number_inset,
            item_padding_right,
            item_min_width,
            number_gap,
            number_scale,
            selected_bar_width,
            selected_bar_height_em,
            selected_bar_inside,
            item_corners_follow_card,
            justify_rows,
            trim_hidden_preedit_gap,
            page_arrow_style,
            preedit_margin_dip
        );
    }
}

pub(super) fn resolve(base: Option<&str>, package: &SkinSummary) -> Option<ResolvedSkinLayout> {
    let base = base?;
    let (_, factory) = factories().iter().find(|(id, _)| *id == base)?;
    let window = factory["candidate_window"]
        .as_table()
        .expect("出厂候选窗表");
    let common = read_geometry(window).expect("出厂公共几何");
    let geometry = |direction, declared: &SkinGeometry| {
        // 出厂清单未写这两个键时，来源 CandSkinTokens 仍有固定初值。
        let mut result = SkinGeometry::default();
        result.overlay(&common);
        if let Some(layer) = window.get(direction) {
            result.overlay(
                &read_geometry(layer.as_table().expect("出厂排列表")).expect("出厂排列几何"),
            );
        }
        result.overlay(&package.geometry);
        result.overlay(declared);
        result
    };
    let number = |key| {
        window[key]
            .as_float()
            .or_else(|| window[key].as_integer().map(|n| n as f64))
            .expect("出厂尺寸")
    };
    // canvas 不是可选内置外观，来源按 fluent 解析它的三个结构开关。
    let switches = if base == "canvas" {
        factories()
            .iter()
            .find(|(id, _)| *id == "fluent")
            .expect("出厂 Fluent 清单")
            .1["candidate_window"]
            .as_table()
            .expect("出厂 Fluent 候选窗表")
    } else {
        window
    };
    let flag = |key| switches.get(key).and_then(Value::as_bool).unwrap_or(false);
    Some(ResolvedSkinLayout {
        corner_radius_dip: package
            .corner_radius_dip
            .unwrap_or_else(|| number("corner_radius_dip")),
        border_width_dip: package
            .border_width_dip
            .unwrap_or_else(|| number("border_width_dip")),
        item_corner_radius_dip: package
            .item_corner_radius_dip
            .unwrap_or_else(|| number("item_corner_radius_dip")),
        shadow: package.shadow.clone().or_else(|| {
            window
                .get("shadow")
                .and_then(Value::as_str)
                .map(str::to_owned)
        }),
        page_arrows: package.page_arrows.unwrap_or_else(|| flag("page_arrows")),
        fit_content: package.fit_content.unwrap_or_else(|| flag("fit_content")),
        expand_panel: package.expand_panel.unwrap_or_else(|| flag("expand_panel")),
        expand_rows: package.expand_rows.unwrap_or_else(|| {
            window
                .get("expand_rows")
                .and_then(Value::as_integer)
                .unwrap_or(4) as u8
        }),
        geometry: {
            let mut result = common.clone();
            result.overlay(&package.geometry);
            result
        },
        horizontal: geometry("horizontal", &package.horizontal),
        vertical: geometry("vertical", &package.vertical),
    })
}

#[cfg(test)]
mod tests;
