use super::*;
use serde_json::json;

fn resolved(theme: GlobalTheme, picked: Option<&str>) -> serde_json::Value {
    let mut custom = CustomTheme::default();
    custom.candidate_colors.selected_text = picked.map(str::to_owned);
    serde_json::to_value(resolve(
        theme,
        &custom,
        false,
        CandidateLayout::Horizontal,
        None,
    ))
    .unwrap()
}

#[test]
fn touch_selected_text_returns_only_the_explicit_custom_picker() {
    assert_eq!(
        resolved(GlobalTheme::Custom, Some("#7b3fbf"))["touch_selected_text"],
        json!("#7B3FBF")
    );
    for theme in GlobalTheme::ALL {
        if theme != GlobalTheme::Custom {
            assert!(resolved(theme, Some("#7B3FBF"))
                .get("touch_selected_text")
                .is_none());
        }
    }
}

#[test]
fn touch_selected_text_keeps_the_existing_strip_palette_without_a_valid_picker() {
    for picked in [None, Some("invalid"), Some("#12345"), Some("#12345678")] {
        assert!(resolved(GlobalTheme::Custom, picked)
            .get("touch_selected_text")
            .is_none());
    }
}
