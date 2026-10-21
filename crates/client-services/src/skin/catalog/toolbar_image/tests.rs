use super::*;

#[test]
fn preview_layout_mirror_matches_shared_parser_and_layout() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../shared/skins/toolbar-image-layout-cases.json"
    )))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let table = toml::from_str(case["manifest"].as_str().unwrap()).unwrap();
        let skin = read(&table, &mut Vec::new()).unwrap().unwrap();
        assert_eq!(serde_json::to_value(&skin).unwrap(), case["skin"]);
        let shown = case["shown"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| (pair[0].as_str().unwrap(), pair[1].as_str().unwrap()));
        assert_eq!(
            serde_json::to_value(skin.layout(shown)).unwrap(),
            case["expected"]
        );
    }
}

fn parse(extra: &str) -> Result<(SkinToolbarImage, Vec<String>), String> {
    let table = toml::from_str(&format!(
        "[toolbar]\nmode='image'\nimage='bar.png'\nimage_width_dip=168\nimage_height_dip=48\n{extra}"
    ))
    .unwrap();
    let mut notices = Vec::new();
    Ok((read(&table, &mut notices)?.unwrap(), notices))
}

#[test]
fn fixed_buttons_do_not_consume_flow_and_hidden_buttons_leave_no_gap() {
    let (skin, _) = parse(
        "[toolbar.button_defaults]\nwidth_dip=30\nheight_dip=20\nbackground='button.png'\n\
         [toolbar.button_flow]\nx_dip=5\ny_dip=7\ngap_dip=3\n\
         [toolbar.buttons.status]\nx_dip=170\ny_dip=1\nwidth_dip=40\n\
         [toolbar.buttons.fullwidth]\nvisible=false\n\
         [toolbar.buttons.punctuation]\nx_dip=99\nheight_dip=25\n",
    )
    .unwrap();
    let layout = skin.layout([
        ("status", "cn"),
        ("fullwidth", "full"),
        ("punctuation", "cn"),
        ("settings", ""),
    ]);
    assert_eq!((layout.width_dip, layout.height_dip), (210.0, 48.0));
    let slots = &layout.buttons;
    assert_eq!(slots.len(), 3);
    assert_eq!((slots[0].x_dip, slots[0].y_dip), (170.0, 1.0));
    assert_eq!((slots[1].x_dip, slots[1].y_dip), (5.0, 7.0));
    assert_eq!((slots[2].x_dip, slots[2].y_dip), (38.0, 7.0));
    assert_eq!(slots[1].height_dip, 25.0);
    assert_eq!(slots[1].icon_size_dip, 20.5);
    assert_eq!(slots[2].background.as_deref(), Some("button.png"));
    // 不在 shown 中的表情按钮不能被皮肤主动开启。
    assert!(!slots.iter().any(|slot| slot.key == "emoji"));
}

#[test]
fn vertical_flow_and_state_image_precedence_match_upstream() {
    let (skin, _) = parse(
        "[toolbar.button_defaults]\nicon_size_dip=18\nbackground='default.webp'\n\
         [toolbar.button_flow]\ndirection='vertical'\ngap_dip=4\n\
         [toolbar.buttons.status]\nimage='status.gif'\nbackground='status-bg.png'\n\
         [toolbar.buttons.status.images]\ncn='cn.apng'\nen='en.PNG'\n",
    )
    .unwrap();
    let layout = skin.layout([("status", "cn"), ("emoji", "")]);
    assert_eq!(layout.buttons[0].image.as_deref(), Some("cn.apng"));
    assert_eq!(
        layout.buttons[0].background.as_deref(),
        Some("status-bg.png")
    );
    assert_eq!(layout.buttons[0].icon_size_dip, 18.0);
    assert_eq!(layout.buttons[1].y_dip, 30.0);
    assert_eq!(layout.height_dip, 56.0);
    assert_eq!(
        skin.layout([("status", "ja")]).buttons[0].image.as_deref(),
        Some("status.gif")
    );
    assert!(layout.buttons[1].image.is_none());
}

#[test]
fn missing_coordinates_without_flow_hide_buttons_and_report_partial_pairs() {
    let (skin, notices) =
        parse("[toolbar.buttons.status]\nx_dip=5\n[toolbar.buttons.emoji]\nx_dip=0\ny_dip=0\n")
            .unwrap();
    let layout = skin.layout([("status", "cn"), ("emoji", ""), ("settings", "")]);
    assert_eq!(notices.len(), 1);
    assert_eq!(layout.buttons.len(), 1);
    assert_eq!(layout.buttons[0].key, "emoji");
    assert_eq!(layout.buttons[0].width_dip, 26.0);
    assert_eq!(layout.buttons[0].icon_size_dip, 26.0 * 0.82);
}

#[test]
fn unknown_buttons_are_ignored_but_unknown_state_paths_are_validated() {
    let (skin, notices) = parse(
        "[toolbar.buttons]\nfuture_button=42\n[toolbar.buttons.status.images]\nfuture='future.png'\ncn='cn.png'\n",
    )
    .unwrap();
    assert_eq!(notices.len(), 2);
    assert_eq!(
        skin.image_paths().collect::<Vec<_>>(),
        ["bar.png", "cn.png"]
    );
    assert!(parse("[toolbar.buttons.status.images]\nfuture='../outside.png'\n").is_err());
    assert!(parse("[toolbar.buttons.emoji.images]\n").is_err());
    let (skin, _) = parse("[toolbar.button_flow]\n").unwrap();
    assert_eq!(
        skin.layout([("unknown", ""), ("status", "cn"), ("status", "en")])
            .buttons
            .len(),
        1
    );
}

#[test]
fn bar_mode_validates_then_discards_unused_image_fields() {
    let table = toml::from_str("[toolbar]\nimage='missing.png'\nimage_width_dip=0\n").unwrap();
    let mut notices = Vec::new();
    assert_eq!(read(&table, &mut notices).unwrap(), None);
    assert_eq!(notices.len(), 1);
    for invalid in [
        "mode=1",
        "mode='other'",
        "image='../outside.png'",
        "image='bar.jpg'",
        "image_width_dip=-1",
        "image_height_dip=2001",
        "button_defaults=false",
        "buttons=[]",
        "button_flow=[]",
        "button_defaults={width_dip=0}",
    ] {
        let table = toml::from_str(&format!("[toolbar]\n{invalid}\n")).unwrap();
        assert!(read(&table, &mut Vec::new()).is_err(), "{invalid}");
    }
}

#[test]
fn image_mode_requires_positive_dimensions_and_bounded_button_fields() {
    for invalid in [
        "image='bar.png'",
        "image_width_dip=1\nimage_height_dip=1",
        "image='bar.png'\nimage_width_dip=0\nimage_height_dip=1",
        "image='bar.png'\nimage_width_dip=1\nimage_height_dip=nan",
    ] {
        let table = toml::from_str(&format!("[toolbar]\nmode='image'\n{invalid}")).unwrap();
        assert!(read(&table, &mut Vec::new()).is_err(), "{invalid}");
    }
    for invalid in [
        "visible=1",
        "x_dip=-1",
        "y_dip=2001",
        "width_dip=0",
        "height_dip=501",
        "icon_size_dip=inf",
        "background='C:/bar.png'",
        "image='bar.svg'",
        "images=[]",
    ] {
        assert!(
            parse(&format!("[toolbar.buttons.status]\n{invalid}")).is_err(),
            "{invalid}"
        );
    }
    for invalid in ["direction='diagonal'", "gap_dip=501", "y_dip=-1"] {
        assert!(
            parse(&format!("[toolbar.button_flow]\n{invalid}")).is_err(),
            "{invalid}"
        );
    }
}
