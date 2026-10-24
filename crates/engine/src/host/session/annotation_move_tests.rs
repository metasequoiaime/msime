use super::*;
use crate::host::tests::{generated_mode_options, helpcode_fixture};
use crate::ime::personal_rerank::allocations;

// 冻结 d2434c7f88 的宿主注释复制路径；预编辑沿用当前宿主显示契约。
fn frozen_snapshot(session: &Session) -> Result<EngineSnapshot> {
    let value = session.inner.snapshot_for_host(true);
    let helpcode_scheme = value.scheme.helpcode();
    let uppercase_all = value.scheme == SchemeType::Quanpin;
    let keymap = session.helpcode_keymap.as_deref().filter(|_| {
        session.helpcode_enabled && helpcode_scheme && !value.local_mode.annotates_rows()
    });
    let count = value.candidates.len();
    let display_reading = if !value.scheme.draws_reading() {
        String::new()
    } else if value.scheme == SchemeType::JapaneseRomaji
        && value.local_mode == LocalInputMode::None
        && !value.dedicated_english
    {
        let mut reading = String::new();
        romaji_display_reading_into(&value.raw_segmentation, &mut reading);
        reading
    } else {
        value.normalized_segmentation.clone()
    };
    let mut output = EngineSnapshot {
        prediction: false,
        local_mode: value.local_mode.name().to_owned(),
        spelling_symbols: value.spelling_symbols,
        dedicated_english: value.dedicated_english,
        nine_key: session.key_grid == Some(KeyGrid::NineKey),
        key_grid: session.key_grid,
        nine_key_spellings: value.nine_key_spellings,
        nine_key_reading: value.nine_key_reading,
        nine_key_single_character: value.nine_key_single_character,
        nine_key_strokes: value.nine_key_strokes,
        grid_composing: session.inner.grid_composing(),
        microsoft_shuangpin: session.microsoft_shuangpin,
        shuangpin_profile: session.shuangpin_profile.clone(),
        preedit: value.preedit,
        display_reading,
        reading: if value.scheme.draws_reading() {
            value.normalized_segmentation
        } else {
            String::new()
        },
        editing_text: value.editing_text,
        caret_position: value.caret_position,
        segment_raw_boundaries: session
            .inner
            .segment_raw_boundaries()
            .into_iter()
            .map(|boundary| boundary as u64)
            .collect(),
        candidates: Vec::with_capacity(count),
        candidate_codes: Vec::with_capacity(count),
        scheme: value.scheme as u8,
        answered_by_pinyin_fallback: value.answered_by_pinyin_fallback,
        wubi_unique_four_code: value.wubi_unique_four_code,
        candidate_annotations: Vec::with_capacity(count),
        candidate_sources: Vec::with_capacity(count),
        multilingual_conversion: None,
        candidate_positions: Vec::with_capacity(count),
        candidate_corrected: Vec::with_capacity(count),
        candidate_answers_key: Vec::with_capacity(count),
        candidate_static_evidence: Vec::with_capacity(count),
        candidate_list_open: value.candidate_list_open,
        conversion: value.conversion,
        conversion_focus_start: value.conversion_focus.0,
        conversion_focus_end: value.conversion_focus.1,
    };
    for (index, candidate) in value.candidates.into_iter().enumerate() {
        let mut annotation = value
            .candidate_annotations
            .get(index)
            .cloned()
            .unwrap_or_else(|| candidate.corrected_from.to_string());
        if let Some(keymap) = keymap {
            if !session.show_helpcode {
                let helpcode = compute_helpcodes(&candidate.word, uppercase_all, keymap);
                if !helpcode.is_empty() && annotation == helpcode {
                    annotation = candidate.corrected_from.to_string();
                }
            } else if annotation.is_empty() && candidate.source == CandidateSource::Generated {
                annotation = compute_helpcodes(&candidate.word, uppercase_all, keymap);
            }
        }
        if value.local_mode == LocalInputMode::None
            && !value.dedicated_english
            && value.scheme == SchemeType::Wubi
            && matches!(
                candidate.scheme,
                SchemeType::Quanpin | SchemeType::Shuangpin | SchemeType::Wubi
            )
        {
            if let Some(code) = session.inner.candidate_wubi_code(&candidate.word) {
                if !annotation.contains(code) {
                    if !annotation.is_empty() {
                        annotation.push(' ');
                    }
                    annotation.push_str(code);
                }
            }
        }
        output.candidate_annotations.push(annotation);
        output.candidate_sources.push(candidate.source as u8);
        output
            .candidate_positions
            .push(candidate.fixed_position as u8);
        output
            .candidate_corrected
            .push(!candidate.corrected_from.is_empty());
        output.candidate_answers_key.push(
            value
                .candidate_answers_key
                .get(index)
                .copied()
                .unwrap_or(false),
        );
        output
            .candidate_static_evidence
            .push(candidate.static_evidence());
        output.candidate_codes.push(candidate.pinyin.into_string());
        output.candidates.push(candidate.word.into_string());
    }
    Ok(output)
}

fn type_bytes(session: &mut Session, text: &[u8]) {
    for &byte in text {
        session.character(byte, false).unwrap();
    }
}

fn compare_snapshot(session: &Session, label: &str) -> EngineSnapshot {
    // 初始化和计数元数据均在计量外；两个构造器各自返回独立拥有的快照。
    session.snapshot().unwrap();
    let copies = session
        .inner
        .snapshot()
        .candidate_annotations
        .iter()
        .filter(|text| !text.is_empty())
        .count();
    let (expected, old_allocations) = allocations::count(|| frozen_snapshot(session).unwrap());
    let (actual, new_allocations) = allocations::count(|| session.snapshot().unwrap());
    assert_eq!(actual, expected, "{label}");
    eprintln!("{label}: {old_allocations} → {new_allocations}，原非空注释 {copies} 行");
    assert!(
        new_allocations + copies <= old_allocations,
        "{label}: 拥有型注释移交没有保留原有分配收益"
    );
    actual
}

#[test]
fn host_snapshot_moves_owned_annotations_without_an_extra_copy() {
    let root = tempfile::tempdir().unwrap();
    let mut options = helpcode_fixture(root.path(), "", "");
    let generated = generated_mode_options(root.path());
    options.local_command = true;
    options.command_table = generated.command_table;
    let mut session = Session::new(&options).unwrap();
    type_bytes(&mut session, b"/h");
    let snapshot = compare_snapshot(&session, "合成指令注释");
    assert_eq!(snapshot.candidate_annotations, ["问候"]);
    session.command(Command::Cancel).unwrap();
    drop(session);
    assert_eq!(snapshot.candidates, ["你好"]);
    assert_eq!(snapshot.candidate_annotations, ["问候"]);
}

#[test]
fn host_snapshot_annotation_move_matches_all_frozen_fields() {
    let extra = "INSERT INTO wubi86 VALUES('wqvb','你好',300);
CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_s VALUES('shang''hao','sh','上好',100);";
    for (scheme, input, mixed) in [
        (SchemeType::Quanpin, b"nihao".as_slice(), false),
        (SchemeType::Shuangpin, b"nihc".as_slice(), false),
        (SchemeType::Wubi, b"wqvb".as_slice(), false),
        (SchemeType::Wubi, b"nihao".as_slice(), true),
        (SchemeType::Quanpin, b"sahnghao".as_slice(), false),
    ] {
        for visible in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let mut options = helpcode_fixture(root.path(), extra, "你=ab\n好=cd\n拟=ef\n上=gh\n");
            options.scheme = scheme as u8;
            options.wubi_mixed_pinyin = mixed;
            options.show_helpcode = visible;
            let mut session = Session::new(&options).unwrap();
            let idle = compare_snapshot(&session, "空快照");
            assert!(idle.candidates.is_empty());
            type_bytes(&mut session, input);
            let label = format!(
                "{scheme:?}/show={visible}/mixed={mixed}/corrected={}",
                input == b"sahnghao"
            );
            let snapshot = compare_snapshot(&session, &label);
            assert!(!snapshot.candidates.is_empty(), "{label}");
        }
    }
    for (input, mode) in [
        (b"/h".as_slice(), "command"),
        (b"@al".as_slice(), "mention"),
        (b"Ccaizi".as_slice(), "chaizi"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut options = helpcode_fixture(root.path(), "", "𢰩=ab\n𢱆=ab\n");
        let generated = generated_mode_options(root.path());
        options.local_command = true;
        options.local_mention = true;
        options.local_chaizi = true;
        options.command_table = generated.command_table;
        options.mention_entries = generated.mention_entries;
        let mut session = Session::new(&options).unwrap();
        if input.starts_with(b"C") {
            session.character(b'C', true).unwrap();
            type_bytes(&mut session, &input[1..]);
        } else {
            type_bytes(&mut session, input);
        }
        let snapshot = compare_snapshot(&session, mode);
        assert_eq!(snapshot.local_mode, mode);
        assert!(!snapshot.candidates.is_empty());
    }
}
