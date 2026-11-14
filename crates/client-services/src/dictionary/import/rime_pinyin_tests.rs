use super::*;

fn read(kind: ImportKind, text: &str) -> ImportReport {
    parse(kind, "rime", text, 64 * 1024).unwrap()
}

#[test]
fn rime_tone_marks_preserve_spelling_separators_weights_and_lines() {
    let report = read(
        ImportKind::Pinyin,
        "# 合成词典\n---\nname: synthetic\n...\n你好\tnǐ hǎo\t123\n绿色\tLǛ sè\t0.5%\n路\tlù\t9\n女儿\tnǚ'ér\t7\n",
    );
    assert_eq!(report.failed, 0);
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.key.as_str(), entry.weight, entry.line))
            .collect::<Vec<_>>(),
        [
            ("ni hao", 123, 5),
            ("lv se", 50, 6),
            ("lu", 9, 7),
            ("nv'er", 7, 8)
        ]
    );
}

#[test]
fn rime_tone_marks_cover_each_vowel_without_merging_u_and_v() {
    for (marked, plain) in [
        ("āáǎà", "aaaa"),
        ("ēéěè", "eeee"),
        ("īíǐì", "iiii"),
        ("ōóǒò", "oooo"),
        ("ūúǔù", "uuuu"),
        ("üǖǘǚǜ", "vvvvv"),
        ("ńňǹḿê", "nnnme"),
    ] {
        let report = read(ImportKind::Pinyin, &format!("合成\t{marked}\t10\n"));
        assert_eq!(report.entries[0].key, plain);
    }
}

#[test]
fn rime_tone_marks_quick_phrases_use_concatenated_codes() {
    let report = read(
        ImportKind::QuickPhrase,
        "你好\tnǐ hǎo\t123\n女儿\tnǚ'ér\t7\n",
    );
    assert_eq!(report.failed, 0);
    assert_eq!(report.entries[0].key, "nihao");
    assert_eq!(report.entries[1].key, "nver");
    assert_eq!(report.entries[0].value, "你好");
    let long_code = "ā".repeat(33);
    let limited = read(
        ImportKind::QuickPhrase,
        &format!("过长\t{long_code}\n合成\thecheng\n"),
    );
    assert_eq!(limited.entries.len(), 1);
    assert_eq!(limited.first_failures[0].issue, ImportIssue::KeyTooLong);
}

#[test]
fn rime_tone_marks_leave_other_formats_and_alphabets_strict() {
    for format in ["standard", "windows"] {
        let text = if format == "windows" {
            "nǐ\t你\n"
        } else {
            "你\tnǐ\n"
        };
        assert_eq!(
            parse(ImportKind::Pinyin, format, text, 1024),
            Err(ImportError::NoUsableRows)
        );
    }
    for kind in [ImportKind::Wubi, ImportKind::English, ImportKind::Cantonese] {
        assert_eq!(
            parse(kind, "rime", "合成\tnǐ\n", 1024),
            Err(ImportError::NoUsableRows)
        );
    }
    for code in ["ni3", "nǐ!", "你好", "nǐ🙂", "nǐ/hao"] {
        let report = read(
            ImportKind::Pinyin,
            &format!("错误\t{code}\n合成\thecheng\n"),
        );
        assert_eq!(report.failed, 1, "{code}");
        assert_eq!(report.first_failures[0].issue, ImportIssue::KeyAlphabet);
    }
}
