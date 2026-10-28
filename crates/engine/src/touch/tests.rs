use super::*;

/// QWERTY 的 26 个字母键，键宽 10、键高 14，每排比上一排右移半个键。
fn qwerty_keys() -> [KeyRect; 26] {
    let mut keys = [[0.0; 4]; 26];
    for (letters, left, y) in [
        ("qwertyuiop", 5.0, 7.0),
        ("asdfghjkl", 10.0, 21.0),
        ("zxcvbnm", 20.0, 35.0),
    ] {
        for (column, letter) in letters.bytes().enumerate() {
            keys[usize::from(letter - b'a')] = [left + 10.0 * column as f32, y, 10.0, 14.0];
        }
    }
    keys
}

fn qwerty() -> KeyGeometry {
    KeyGeometry::new(7, &qwerty_keys(), None, &[]).unwrap()
}

fn center(key: u8) -> (f32, f32) {
    let [x, y, _, _] = qwerty_keys()[usize::from(key - b'a')];
    (x, y)
}

#[test]
fn touches_are_offsets_from_the_pressed_key_in_twentieths() {
    let geometry = qwerty();
    let (x, y) = center(b'g');
    assert_eq!(
        geometry.touch(b'g', x, y),
        Some(KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0
        })
    );
    // 半个键宽、四分之一个键高：键宽 10、键高 14 归一后是 10 和 5 个单位。
    assert_eq!(
        geometry.touch(b'g', x + 5.0, y - 3.5),
        Some(KeyTouch {
            dx: 10,
            dy: -5,
            copy: 0
        })
    );
    // 大写字母按小写算；离得再远也钳到 ±1.5 键。
    assert_eq!(
        geometry.touch(b'G', x - 100.0, y + 100.0),
        Some(KeyTouch {
            dx: -30,
            dy: 30,
            copy: 0
        })
    );
}

#[test]
fn a_touch_needs_finite_coordinates_and_a_key_in_the_geometry() {
    let geometry = qwerty();
    let (x, y) = center(b'a');
    assert_eq!(geometry.touch(b'a', f32::NAN, y), None);
    assert_eq!(geometry.touch(b'a', x, f32::INFINITY), None);
    // 有限但归一后远在几何范围之外的落点同样不带触点。
    assert_eq!(geometry.touch(b'a', 1.0e36, y), None);
    // 没有 `;` 键的几何里 `;` 没有触点，数字也没有。
    assert_eq!(geometry.touch(b';', x, y), None);
    assert_eq!(geometry.touch(b'1', x, y), None);
    let with_semicolon =
        KeyGeometry::new(7, &qwerty_keys(), Some([100.0, 21.0, 10.0, 14.0]), &[]).unwrap();
    assert!(with_semicolon.has_semicolon());
    assert!(!qwerty().has_semicolon());
    assert_eq!(
        with_semicolon.touch(b';', 100.0, 21.0),
        Some(KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0
        })
    );
}

#[test]
fn geometry_is_validated_as_a_whole() {
    let keys = qwerty_keys();
    assert_eq!(
        KeyGeometry::new(0, &keys, None, &[]),
        Err(KeyGeometryError::ZeroId)
    );
    let mut flat = keys;
    flat[3][3] = 0.0;
    assert_eq!(
        KeyGeometry::new(1, &flat, None, &[]),
        Err(KeyGeometryError::InvalidRect)
    );
    let mut infinite = keys;
    infinite[0][0] = f32::INFINITY;
    assert_eq!(
        KeyGeometry::new(1, &infinite, None, &[]),
        Err(KeyGeometryError::InvalidRect)
    );
    assert_eq!(
        KeyGeometry::new(1, &keys, Some([1.0, 1.0, -1.0, 1.0]), &[]),
        Err(KeyGeometryError::InvalidRect)
    );
    // 原始数值都有限，归一后溢出成无穷：键宽 1e-3、键心 1e36。
    let mut tiny = keys;
    for key in &mut tiny {
        key[2] = 1.0e-3;
    }
    tiny[0][0] = 1.0e36;
    assert_eq!(
        KeyGeometry::new(1, &tiny, None, &[]),
        Err(KeyGeometryError::InvalidRect)
    );
    // 归一后有限，但两个键心的距离平方会溢出：键心 ±1e30。
    let mut far = keys;
    far[0][0] = -1.0e30;
    far[1][0] = 1.0e30;
    assert_eq!(
        KeyGeometry::new(1, &far, None, &[]),
        Err(KeyGeometryError::InvalidRect)
    );
    assert_eq!(
        KeyGeometry::new(1, &keys, None, &[(b'g', [1.0e30, 1.0, 10.0, 14.0])]),
        Err(KeyGeometryError::InvalidRect)
    );
    assert_eq!(
        KeyGeometry::new(1, &keys, None, &[(b'G', [1.0, 1.0, 1.0, 1.0])]),
        Err(KeyGeometryError::InvalidDuplicate)
    );
}

fn geometry_json(id: u32, extra: &str) -> String {
    let keys: Vec<String> = qwerty_keys()
        .iter()
        .map(|[x, y, w, h]| format!("[{x},{y},{w},{h}]"))
        .collect();
    format!(
        "{{\"id\":{id},\"keys\":[{}],\"semicolon\":null,\"duplicates\":[]{extra}}}",
        keys.join(",")
    )
}

#[test]
fn the_wire_form_clears_on_an_empty_string_or_id_zero_and_rejects_the_rest() {
    let parsed = KeyGeometry::from_json(geometry_json(9, "").as_bytes())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.id(), 9);
    assert_eq!(
        parsed,
        KeyGeometry::new(9, &qwerty_keys(), None, &[]).unwrap()
    );
    assert_eq!(KeyGeometry::from_json(b""), Ok(None));
    assert_eq!(KeyGeometry::from_json(b"{\"id\":0}"), Ok(None));
    assert_eq!(
        KeyGeometry::from_json(geometry_json(0, "").as_bytes()),
        Ok(None)
    );
    // 多出的字段、不是 26 个键、坏的重复键、不是 JSON：整份拒绝。
    assert!(KeyGeometry::from_json(geometry_json(9, ",\"scale\":2").as_bytes()).is_err());
    assert!(KeyGeometry::from_json(b"{\"id\":3,\"keys\":[[1,1,1,1]]}").is_err());
    assert!(KeyGeometry::from_json(b"not json").is_err());
    let split =
        geometry_json(9, "").replace("\"duplicates\":[]", "\"duplicates\":[[\"gh\",1,1,1,1]]");
    assert!(KeyGeometry::from_json(split.as_bytes()).is_err());
    let negative = geometry_json(9, "").replacen(",10,14]", ",-10,14]", 1);
    assert!(KeyGeometry::from_json(negative.as_bytes()).is_err());
}

fn tuning() -> TouchTuning {
    TouchTuning {
        base: 12.0,
        scale: 8.0,
        sigma_x: 1.0,
        sigma_y: 1.0,
        bias_y: 0.0,
        min_penalty: 4.0,
        max_penalty: 28.0,
        cut: 16.0,
        show_margin: 0.0,
        show_penalty: f64::INFINITY,
    }
}

#[test]
fn the_penalty_follows_the_touch_between_the_two_keys() {
    let geometry = qwerty();
    let tuning = tuning();
    let at = |dx: i8, dy: i8| KeyTouch { dx, dy, copy: 0 };
    // 键心按 g：同排的邻键 h 相距一个键，罚分 B + s·1。
    let centered = geometry.penalty(b'g', at(0, 0), b'h', &tuning).unwrap();
    assert!((centered - 20.0).abs() < 1e-6, "{centered}");
    // 落在 g 和 h 正中间：两边一样远，罚分就是 B。
    let between = geometry.penalty(b'g', at(10, 0), b'h', &tuning).unwrap();
    assert!((between - 12.0).abs() < 1e-6, "{between}");
    // 偏向 h 的那一半比偏向 f 的那一半便宜，与对面那个邻键正好相反。
    let toward_h = geometry.penalty(b'g', at(6, 0), b'h', &tuning).unwrap();
    let away_from_h = geometry.penalty(b'g', at(-6, 0), b'h', &tuning).unwrap();
    assert!(toward_h < centered && centered < away_from_h);
    let toward_f = geometry.penalty(b'g', at(-6, 0), b'f', &tuning).unwrap();
    assert!((toward_f - toward_h).abs() < 1e-6);
    // 越过分界、离 h 比离 g 还近时罚分低于 B，但不低于 P_MIN；离得越远封顶在 P_MAX。
    assert_eq!(
        geometry.penalty(b'g', at(30, 0), b'h', &tuning),
        Some(tuning.min_penalty)
    );
    assert_eq!(
        geometry.penalty(b'g', at(0, 0), b'p', &tuning),
        Some(tuning.max_penalty)
    );
    // 不在几何里的键没有罚分。
    assert_eq!(geometry.penalty(b'g', at(0, 0), b';', &tuning), None);
}

#[test]
fn sigma_and_bias_reshape_the_penalty() {
    let geometry = qwerty();
    let at = |dx: i8, dy: i8| KeyTouch { dx, dy, copy: 0 };
    // 上下限放开，只看形状。
    let open = TouchTuning {
        min_penalty: -100.0,
        max_penalty: 100.0,
        ..tuning()
    };
    // 纵向距离单位放大一倍时，键心按 g、换成上一排的 t 便宜下来，同排的 h 不变。
    let wide_y = TouchTuning {
        sigma_y: 2.0,
        ..open
    };
    let up = geometry.penalty(b'g', at(0, 0), b't', &open).unwrap();
    let up_wide = geometry.penalty(b'g', at(0, 0), b't', &wide_y).unwrap();
    assert!(up_wide < up, "{up_wide} vs {up}");
    let same_row = geometry.penalty(b'g', at(5, 0), b'h', &open).unwrap();
    let same_row_wide = geometry.penalty(b'g', at(5, 0), b'h', &wide_y).unwrap();
    assert!((same_row - same_row_wide).abs() < 1e-6);
    // 偏置把每个键的瞄准点往下挪：键心正中的落点像是往上偏了，上一排变便宜、下一排变贵。
    let biased = TouchTuning {
        bias_y: 0.2,
        ..open
    };
    let above = |tuning: &TouchTuning| geometry.penalty(b'g', at(0, 0), b'y', tuning).unwrap();
    let below = |tuning: &TouchTuning| geometry.penalty(b'g', at(0, 0), b'b', tuning).unwrap();
    assert!(above(&biased) < above(&open));
    assert!(below(&biased) > below(&open));
}

/// 分离键盘：左半边是 QWERTY 的左五列，右半边整体右移 60，G、V 在右半边内侧再各出现一次。
fn split() -> KeyGeometry {
    let mut keys = qwerty_keys();
    for letter in b"yuiophjklbnm" {
        keys[usize::from(letter - b'a')][0] += 60.0;
    }
    KeyGeometry::new(
        3,
        &keys,
        None,
        &[
            (b'g', [110.0, 21.0, 10.0, 14.0]),
            (b'v', [100.0, 35.0, 10.0, 14.0]),
        ],
    )
    .unwrap()
}

#[test]
fn split_layouts_pick_the_nearest_copy_and_drop_neighbours_across_the_gap() {
    let geometry = split();
    // 右半边的 G 是第二个实例。
    let right_g = geometry.touch(b'g', 110.0, 21.0).unwrap();
    assert_eq!(right_g.copy, 1);
    assert_eq!(geometry.touch(b'g', 50.0, 21.0).unwrap().copy, 0);
    // 平面表说 t 和 y、g 和 h 相邻，分在两半时不算；右半边的 G 与 H 挨着。
    let t = geometry.touch(b't', 45.0, 7.0).unwrap();
    assert!(!geometry.adjacent(b't', t, b'y'));
    assert!(geometry.adjacent(b't', t, b'r'));
    let left_g = geometry.touch(b'g', 50.0, 21.0).unwrap();
    assert!(!geometry.adjacent(b'g', left_g, b'h'));
    assert!(geometry.adjacent(b'g', right_g, b'h'));
    // 罚分也按离落点最近的实例算：右半边按 G 偏向 H，与普通布局里一样便宜。
    let toward_h = KeyTouch {
        dx: 6,
        dy: 0,
        copy: 1,
    };
    let split_penalty = geometry.penalty(b'g', toward_h, b'h', &tuning()).unwrap();
    let qwerty_penalty = qwerty()
        .penalty(
            b'g',
            KeyTouch {
                dx: 6,
                dy: 0,
                copy: 0,
            },
            b'h',
            &tuning(),
        )
        .unwrap();
    assert!((split_penalty - qwerty_penalty).abs() < 1e-6);
}

#[test]
fn the_fingerprint_follows_the_content_not_the_id() {
    // 内容相同、分别构造（含 -0.0 与 0.0）的几何指纹相同，代次不进指纹。
    let mut signed_zero = qwerty_keys();
    signed_zero[0][1] = -0.0;
    let mut zero = qwerty_keys();
    zero[0][1] = 0.0;
    let left = KeyGeometry::new(7, &signed_zero, None, &[]).unwrap();
    let right = KeyGeometry::new(8, &zero, None, &[]).unwrap();
    assert_eq!(left.fingerprint(), right.fingerprint());
    // 同一个代次挪了一个键、加了 `;` 或重复键，指纹都变。
    let mut moved = qwerty_keys();
    moved[usize::from(b'r' - b'a')][0] += 5.0;
    let base = qwerty();
    for changed in [
        KeyGeometry::new(7, &moved, None, &[]).unwrap(),
        KeyGeometry::new(7, &qwerty_keys(), Some([100.0, 21.0, 10.0, 14.0]), &[]).unwrap(),
        KeyGeometry::new(
            7,
            &qwerty_keys(),
            None,
            &[(b'g', [110.0, 21.0, 10.0, 14.0])],
        )
        .unwrap(),
    ] {
        assert_eq!(changed.id(), base.id());
        assert_ne!(changed.fingerprint(), base.fingerprint());
    }
}

/// 默认计价参数是在 Android 模拟器导出的几何上扫参定下的（PR1b，见 `.agents/notes/implemented/feature/2026-10-27-shuangpin-touch-correction.md`）。改任何一项都要重跑扫参和 `baseline-shuangpin-touch-*.json`，这里钉住，免得顺手改了却没重跑。
#[test]
fn the_default_tuning_is_the_swept_one() {
    assert_eq!(
        TouchTuning::default(),
        TouchTuning {
            base: 8.0,
            scale: 12.0,
            sigma_x: 1.0,
            sigma_y: 1.0,
            bias_y: 0.0,
            min_penalty: 4.0,
            max_penalty: 28.0,
            cut: 16.0,
            show_margin: 6.0,
            show_penalty: 14.0,
        }
    );
}
