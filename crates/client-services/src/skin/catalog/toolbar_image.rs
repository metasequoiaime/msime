//! 图片工具栏的清单契约与布局；宿主只提供当前显示的按钮和状态。

use super::{number_in, safe_resource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toml::{Table, Value};

pub const TOOLBAR_BUTTON_KEYS: [&str; 7] = [
    "status",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "screen_keyboard",
    "settings",
];

fn states(key: &str) -> Option<&'static [&'static str]> {
    match key {
        "status" => Some(&["cn", "en", "ja", "caps", "en-candidate"]),
        "fullwidth" => Some(&["full", "half"]),
        "punctuation" => Some(&["cn", "en"]),
        "character_set" => Some(&["simplified", "traditional"]),
        "emoji" | "screen_keyboard" | "settings" => Some(&[]),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbarButtonLook {
    pub width_dip: Option<f64>,
    pub height_dip: Option<f64>,
    /// `None` 表示由绘制端取按钮短边的 0.82 倍。
    pub icon_size_dip: Option<f64>,
    pub background: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbarImageButton {
    pub visible: Option<bool>,
    pub x_dip: Option<f64>,
    pub y_dip: Option<f64>,
    #[serde(flatten)]
    pub look: SkinToolbarButtonLook,
    pub image: Option<String>,
    pub images: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolbarFlowDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbarButtonFlow {
    pub x_dip: f64,
    pub y_dip: f64,
    pub direction: ToolbarFlowDirection,
    pub gap_dip: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinToolbarImage {
    pub image: String,
    pub image_width_dip: f64,
    pub image_height_dip: f64,
    pub button_defaults: SkinToolbarButtonLook,
    pub button_flow: Option<SkinToolbarButtonFlow>,
    pub buttons: BTreeMap<String, SkinToolbarImageButton>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolbarImageButtonSlot {
    pub key: String,
    pub x_dip: f64,
    pub y_dip: f64,
    pub width_dip: f64,
    pub height_dip: f64,
    /// 已解析为实际字号，宿主不再重复默认值规则。
    pub icon_size_dip: f64,
    pub background: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolbarImageLayout {
    pub width_dip: f64,
    pub height_dip: f64,
    pub buttons: Vec<ToolbarImageButtonSlot>,
}

impl SkinToolbarImage {
    /// 无磁盘 I/O 的宿主桥：重新校验缓存 JSON 的边界，再按同一规则排列。
    pub fn checked_layout<'a>(
        &self,
        shown: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<ToolbarImageLayout, &'static str> {
        let bounded = |value: f64, max: f64, positive: bool| {
            value.is_finite() && value >= 0.0 && value <= max && (!positive || value > 0.0)
        };
        let valid_look = |look: &SkinToolbarButtonLook| {
            [look.width_dip, look.height_dip, look.icon_size_dip]
                .into_iter()
                .flatten()
                .all(|value| bounded(value, 500.0, true))
        };
        if !bounded(self.image_width_dip, 2000.0, true)
            || !bounded(self.image_height_dip, 2000.0, true)
            || !valid_look(&self.button_defaults)
            || self.button_flow.as_ref().is_some_and(|flow| {
                !bounded(flow.x_dip, 2000.0, false)
                    || !bounded(flow.y_dip, 2000.0, false)
                    || !bounded(flow.gap_dip, 500.0, false)
            })
            || self.buttons.len() > TOOLBAR_BUTTON_KEYS.len()
            || self.buttons.iter().any(|(key, button)| {
                let Some(known) = states(key) else {
                    return true;
                };
                !valid_look(&button.look)
                    || [button.x_dip, button.y_dip]
                        .into_iter()
                        .flatten()
                        .any(|value| !bounded(value, 2000.0, false))
                    || button
                        .images
                        .keys()
                        .any(|state| !known.contains(&state.as_str()))
            })
            || self
                .image_paths()
                .any(|path| image_value(&Value::String(path.to_owned()), "image").is_err())
        {
            return Err("invalid toolbar image layout");
        }
        Ok(self.layout(shown))
    }

    /// `shown` 必须来自宿主当前开关；皮肤只能继续隐藏，不能重新开启按钮。
    /// 固定坐标按钮不消耗自动排列位置，缺少任一坐标时整对坐标交给 flow。
    pub fn layout<'a>(
        &self,
        shown: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> ToolbarImageLayout {
        let none = SkinToolbarImageButton::default();
        let mut output = ToolbarImageLayout {
            width_dip: self.image_width_dip,
            height_dip: self.image_height_dip,
            buttons: Vec::new(),
        };
        let mut x = self.button_flow.as_ref().map_or(0.0, |flow| flow.x_dip);
        let mut y = self.button_flow.as_ref().map_or(0.0, |flow| flow.y_dip);
        for (key, state) in shown {
            // 未知按钮不能占用 flow；重复请求也不创建重叠命中区。
            if states(key).is_none() || output.buttons.iter().any(|slot| slot.key == key) {
                continue;
            }
            let button = self.buttons.get(key).unwrap_or(&none);
            if button.visible == Some(false) {
                continue;
            }
            let width = button
                .look
                .width_dip
                .or(self.button_defaults.width_dip)
                .unwrap_or(26.0);
            let height = button
                .look
                .height_dip
                .or(self.button_defaults.height_dip)
                .unwrap_or(26.0);
            let (button_x, button_y) = if let (Some(x), Some(y)) = (button.x_dip, button.y_dip) {
                (x, y)
            } else if let Some(flow) = &self.button_flow {
                let position = (x, y);
                match flow.direction {
                    ToolbarFlowDirection::Horizontal => x += width + flow.gap_dip,
                    ToolbarFlowDirection::Vertical => y += height + flow.gap_dip,
                }
                position
            } else {
                continue;
            };
            output.width_dip = output.width_dip.max(button_x + width);
            output.height_dip = output.height_dip.max(button_y + height);
            output.buttons.push(ToolbarImageButtonSlot {
                key: key.to_owned(),
                x_dip: button_x,
                y_dip: button_y,
                width_dip: width,
                height_dip: height,
                icon_size_dip: button
                    .look
                    .icon_size_dip
                    .or(self.button_defaults.icon_size_dip)
                    .unwrap_or(width.min(height) * 0.82),
                background: button
                    .look
                    .background
                    .clone()
                    .or_else(|| self.button_defaults.background.clone()),
                image: button
                    .images
                    .get(state)
                    .cloned()
                    .or_else(|| button.image.clone()),
            });
        }
        output
    }

    pub(super) fn image_paths(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.image.as_str())
            .chain(self.button_defaults.background.as_deref())
            .chain(self.buttons.values().flat_map(|button| {
                button
                    .look
                    .background
                    .as_deref()
                    .into_iter()
                    .chain(button.image.as_deref())
                    .chain(button.images.values().map(String::as_str))
            }))
    }
}

fn number(table: &Table, key: &str, max: f64, positive: bool) -> Result<Option<f64>, String> {
    table
        .get(key)
        .map(|value| {
            number_in(value, 0.0..=max)
                .filter(|number| !positive || *number > 0.0)
                .ok_or_else(|| format!("invalid toolbar {key}"))
        })
        .transpose()
}

fn image_value(value: &Value, key: &str) -> Result<String, String> {
    let invalid = || format!("invalid toolbar {key}");
    let path = value.as_str().ok_or_else(invalid)?;
    let suffix = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if !safe_resource(path, 256) || !matches!(suffix.as_str(), "png" | "apng" | "gif" | "webp") {
        return Err(invalid());
    }
    Ok(path.to_owned())
}

fn image(table: &Table, key: &str) -> Result<Option<String>, String> {
    table
        .get(key)
        .map(|value| image_value(value, key))
        .transpose()
}

fn look(table: &Table) -> Result<SkinToolbarButtonLook, String> {
    Ok(SkinToolbarButtonLook {
        width_dip: number(table, "width_dip", 500.0, true)?,
        height_dip: number(table, "height_dip", 500.0, true)?,
        icon_size_dip: number(table, "icon_size_dip", 500.0, true)?,
        background: image(table, "background")?,
    })
}

pub(super) fn read(
    table: &Table,
    notices: &mut Vec<String>,
) -> Result<Option<SkinToolbarImage>, String> {
    let Some(toolbar) = table.get("toolbar") else {
        return Ok(None);
    };
    let toolbar = toolbar.as_table().ok_or("invalid toolbar")?;
    let mode = match toolbar.get("mode") {
        None => "bar",
        Some(value) => value
            .as_str()
            .filter(|mode| matches!(*mode, "bar" | "image"))
            .ok_or("invalid toolbar mode")?,
    };
    let background = image(toolbar, "image")?;
    let width = number(toolbar, "image_width_dip", 2000.0, false)?.unwrap_or(0.0);
    let height = number(toolbar, "image_height_dip", 2000.0, false)?.unwrap_or(0.0);
    let mut buttons = BTreeMap::new();
    if let Some(value) = toolbar.get("buttons") {
        for (key, value) in value.as_table().ok_or("invalid toolbar buttons")? {
            let Some(states) = states(key) else {
                notices.push(format!("没有 toolbar.buttons.{key} 这个按钮，已忽略"));
                continue;
            };
            let table = value.as_table().ok_or("invalid toolbar button")?;
            let visible = table
                .get("visible")
                .map(|value| value.as_bool().ok_or("invalid toolbar visible"))
                .transpose()?;
            let mut images = BTreeMap::new();
            if let Some(value) = table.get("images") {
                let declared = value.as_table().ok_or("invalid toolbar images")?;
                if states.is_empty() {
                    return Err("toolbar button has no image states".into());
                }
                for (state, value) in declared {
                    let path = image_value(value, state)?;
                    if states.contains(&state.as_str()) {
                        images.insert(state.clone(), path);
                    } else {
                        notices.push(format!(
                            "toolbar.buttons.{key}.images 里的 {state} 不是这个按钮的状态，已忽略"
                        ));
                    }
                }
            }
            buttons.insert(
                key.clone(),
                SkinToolbarImageButton {
                    visible,
                    x_dip: number(table, "x_dip", 2000.0, false)?,
                    y_dip: number(table, "y_dip", 2000.0, false)?,
                    look: look(table)?,
                    image: image(table, "image")?,
                    images,
                },
            );
        }
    }
    let defaults = toolbar
        .get("button_defaults")
        .map(|value| look(value.as_table().ok_or("invalid toolbar button_defaults")?))
        .transpose()?
        .unwrap_or_default();
    let flow = toolbar
        .get("button_flow")
        .map(|value| -> Result<_, String> {
            let table = value.as_table().ok_or("invalid toolbar button_flow")?;
            let direction = match table.get("direction") {
                None => ToolbarFlowDirection::Horizontal,
                Some(Value::String(value)) if value == "horizontal" => {
                    ToolbarFlowDirection::Horizontal
                }
                Some(Value::String(value)) if value == "vertical" => ToolbarFlowDirection::Vertical,
                _ => return Err("invalid toolbar button_flow direction".into()),
            };
            Ok(SkinToolbarButtonFlow {
                x_dip: number(table, "x_dip", 2000.0, false)?.unwrap_or(0.0),
                y_dip: number(table, "y_dip", 2000.0, false)?.unwrap_or(0.0),
                gap_dip: number(table, "gap_dip", 500.0, false)?.unwrap_or(0.0),
                direction,
            })
        })
        .transpose()?;
    if mode == "bar" {
        if background.is_some()
            || !buttons.is_empty()
            || flow.is_some()
            || toolbar.contains_key("button_defaults")
        {
            notices.push("toolbar 的 image、buttons、button_defaults 与 button_flow 只在 mode = \"image\" 时生效，已忽略".into());
        }
        return Ok(None);
    }
    let background = background
        .filter(|_| width > 0.0 && height > 0.0)
        .ok_or("toolbar image mode needs image and positive dimensions")?;
    if flow.is_none() {
        for (key, button) in &buttons {
            if button.x_dip.is_some() != button.y_dip.is_some() {
                notices.push(format!(
                    "toolbar.buttons.{key} 只写了一个坐标且没有 button_flow，这个按钮不会显示"
                ));
            }
        }
    }
    Ok(Some(SkinToolbarImage {
        image: background,
        image_width_dip: width,
        image_height_dip: height,
        button_defaults: defaults,
        button_flow: flow,
        buttons,
    }))
}

#[cfg(test)]
mod tests;
