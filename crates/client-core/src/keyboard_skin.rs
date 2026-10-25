//! 键盘皮肤编辑器的固定目录和显式设计变换；不写偏好，也不替宿主保存编辑历史。

use crate::preferences::TouchKeyboardSkinDesign;
use crate::theme::{luminance, readable_text};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct Template {
    pub title: String,
    pub design: TouchKeyboardSkinDesign,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub title: String,
    pub start: u32,
    pub end: Option<u32>,
    pub key_background: u32,
    pub key_foreground: u32,
    pub accent: u32,
    pub action_background: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Catalog {
    #[serde(rename = "default")]
    pub default_design: TouchKeyboardSkinDesign,
    pub templates: Vec<Template>,
    pub backgrounds: Vec<Background>,
}

pub fn catalog() -> Result<Catalog, &'static str> {
    serde_json::from_str(include_str!(
        "../../../resources/keyboard-skin-presets.json"
    ))
    .map_err(|_| "keyboard_skin_catalog_invalid")
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Catalog {},
    Reset {},
    Template {
        index: usize,
    },
    Background {
        index: usize,
        design: TouchKeyboardSkinDesign,
    },
    OptimizeContrast {
        design: TouchKeyboardSkinDesign,
    },
}

fn contrast(first: u32, second: u32) -> f64 {
    let a = luminance(first);
    let b = luminance(second);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// 只改变明确选择的设计；返回值由宿主选择预览、存库或应用。
pub fn edit(action: Action) -> Result<Value, &'static str> {
    let design = match action {
        Action::Catalog {} => {
            return serde_json::to_value(catalog()?).map_err(|_| "keyboard_skin_catalog_invalid")
        }
        Action::Reset {} => TouchKeyboardSkinDesign::default(),
        Action::Template { index } => {
            catalog()?
                .templates
                .into_iter()
                .nth(index)
                .ok_or("keyboard_skin_invalid")?
                .design
        }
        Action::Background { index, mut design } => {
            if !design.validate() {
                return Err("keyboard_skin_invalid");
            }
            let background = catalog()?
                .backgrounds
                .into_iter()
                .nth(index)
                .ok_or("keyboard_skin_invalid")?;
            design.background = background.start;
            design.gradient_end = background.end;
            design.photo = None;
            design.key_background = background.key_background;
            design.key_foreground = background.key_foreground;
            design.accent = background.accent;
            design.action_background = background.action_background;
            design
        }
        Action::OptimizeContrast { mut design } => {
            if !design.validate() {
                return Err("keyboard_skin_invalid");
            }
            // 与共享网页编辑器相同：按键文字取黑白可读色，强调色取背景和按键上最好的最小对比度。
            let black = contrast(0, design.background).min(contrast(0, design.key_background));
            let white = contrast(0xffffff, design.background)
                .min(contrast(0xffffff, design.key_background));
            design.key_foreground = readable_text(design.key_background);
            design.accent = if black >= white { 0 } else { 0xffffff };
            design
        }
    };
    if !design.validate() {
        return Err("keyboard_skin_invalid");
    }
    serde_json::to_value(design).map_err(|_| "keyboard_skin_invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_catalog_preserves_defaults_and_validates_every_design() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.default_design, TouchKeyboardSkinDesign::default());
        assert_eq!(catalog.templates.len(), 15);
        assert_eq!(catalog.backgrounds.len(), 9);
        assert_eq!(
            catalog.templates[0].design,
            TouchKeyboardSkinDesign::mint_morning()
        );
        for template in &catalog.templates {
            assert!(template.design.validate());
        }
        for index in 0..catalog.backgrounds.len() {
            let original = TouchKeyboardSkinDesign {
                corner_radius: 17.0,
                ..Default::default()
            };
            let next: TouchKeyboardSkinDesign = serde_json::from_value(
                edit(Action::Background {
                    index,
                    design: original,
                })
                .unwrap(),
            )
            .unwrap();
            assert_eq!(next.corner_radius, 17.0);
            assert!(next.photo.is_none());
            assert!(next.validate());
            assert!(contrast(next.key_foreground, next.key_background) >= 4.5);
        }
        assert!(edit(Action::Template { index: usize::MAX }).is_err());
    }

    #[test]
    fn explicit_contrast_change_preserves_other_fields_and_rejects_invalid_designs() {
        let original = TouchKeyboardSkinDesign {
            key_foreground: 0xffffff,
            accent: 0xffffff,
            ..Default::default()
        };
        let mut expected = original.clone();
        expected.key_foreground = readable_text(original.key_background);
        expected.accent = 0;
        let next: TouchKeyboardSkinDesign =
            serde_json::from_value(edit(Action::OptimizeContrast { design: original }).unwrap())
                .unwrap();
        assert_eq!(next, expected);
        assert!(contrast(next.key_foreground, next.key_background) >= 4.5);
        assert!(edit(Action::OptimizeContrast {
            design: TouchKeyboardSkinDesign {
                background: 0x1000000,
                ..Default::default()
            }
        })
        .is_err());
    }
}
