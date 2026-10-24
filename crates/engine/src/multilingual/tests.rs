use super::*;

const ALL: &[Language] = &[Language::Chinese, Language::English, Language::Japanese];

fn edge(start: usize, end: usize, text: &str, language: Language) -> Lexeme {
    Lexeme {
        input: start..end,
        text: text.to_owned(),
        language,
        rank: 0,
    }
}

fn text(path: &Conversion) -> String {
    path.segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect()
}

#[test]
fn converts_a_continuous_input_across_two_language_changes() {
    let input = "alphazhongwenohayou";
    let path = decode(
        input,
        &[
            edge(0, 5, "alpha", Language::English),
            edge(5, 10, "中", Language::Chinese),
            edge(10, 13, "文", Language::Chinese),
            edge(13, 19, "おはよう", Language::Japanese),
        ],
        ALL,
    )
    .expect("完整的三语言路径必须能够转换");
    assert_eq!(text(&path), "alpha中文おはよう");
    assert!(!path.chinese_complete);
    assert_eq!(path.segments.len(), 3);
    assert_eq!(path.segments[1].input, 5..13);
    assert_eq!(path.segments[1].language, Language::Chinese);
}

#[test]
fn backtracks_when_the_longest_local_prefix_cannot_cover_the_tail() {
    let path = decode(
        "abcde",
        &[
            edge(0, 3, "abc", Language::English),
            edge(0, 2, "甲", Language::Chinese),
            edge(2, 5, "おはよう", Language::Japanese),
        ],
        ALL,
    )
    .expect("不能因最长前缀无后续路径而丢掉完整备选");
    assert_eq!(text(&path), "甲おはよう");
}

#[test]
fn excludes_unselected_languages_and_keeps_an_available_complete_alternative() {
    let edges = [
        edge(0, 2, "甲", Language::Chinese),
        edge(2, 5, "おはよう", Language::Japanese),
        edge(2, 5, "def", Language::English),
    ];
    let path = decode("abcde", &edges, &[Language::Chinese, Language::English])
        .expect("未选语言不能挤掉已选语言的路径");
    assert_eq!(text(&path), "甲def");
    assert!(path
        .segments
        .iter()
        .all(|segment| segment.language != Language::Japanese));
}

#[test]
fn does_not_offer_a_partial_or_single_language_conversion_as_mixed() {
    assert!(decode("abcd", &[edge(0, 3, "甲", Language::Chinese)], ALL).is_none());
    assert!(decode("abc", &[edge(0, 3, "甲", Language::Chinese)], ALL).is_none());
    assert!(decode("", &[], ALL).is_none());
}

#[test]
fn rejects_broken_utf8_ranges_and_empty_output_without_losing_a_valid_path() {
    let path = decode(
        "aéz",
        &[
            edge(0, 2, "错误", Language::Chinese),
            edge(0, 99, "错误", Language::Japanese),
            edge(1, 1, "错误", Language::Japanese),
            edge(1, 3, "", Language::Japanese),
            edge(0, 1, "a", Language::English),
            edge(1, 3, "甲", Language::Chinese),
            edge(3, 4, "z", Language::English),
        ],
        ALL,
    )
    .expect("损坏的提供者边不能污染另一条完整路径");
    assert_eq!(text(&path), "a甲z");
}

#[test]
fn preserves_a_whole_word_instead_of_forcing_a_language_change() {
    assert!(decode(
        "women",
        &[
            edge(0, 5, "我们", Language::Chinese),
            edge(0, 2, "我", Language::Chinese),
            edge(2, 5, "men", Language::English),
        ],
        ALL
    )
    .is_none());
}

#[test]
fn keeps_a_mixed_alternative_for_an_equally_short_chinese_sentence() {
    let path = decode(
        "abcd",
        &[
            edge(0, 2, "甲", Language::Chinese),
            edge(2, 4, "乙", Language::Chinese),
            edge(2, 4, "cd", Language::English),
        ],
        ALL,
    )
    .expect("同样完整的中文解释不应删除合理的混合备选");
    assert_eq!(text(&path), "甲cd");
    assert!(path.chinese_complete);
}

#[test]
fn a_shorter_complete_mixed_path_can_beat_a_fragmented_single_language_path() {
    let path = decode(
        "abcdef",
        &[
            edge(0, 2, "甲", Language::Chinese),
            edge(2, 4, "乙", Language::Chinese),
            edge(4, 6, "丙", Language::Chinese),
            edge(0, 4, "abcd", Language::English),
        ],
        ALL,
    )
    .expect("更少词条的完整混合路径仍可用");
    assert_eq!(text(&path), "abcd丙");
    assert!(path.chinese_complete);
}

#[test]
fn accepts_the_input_budget_boundary_and_rejects_one_more_byte() {
    let input = "a".repeat(MAX_INPUT_BYTES);
    let edges = [
        edge(0, MAX_INPUT_BYTES - 1, "alpha", Language::English),
        edge(
            MAX_INPUT_BYTES - 1,
            MAX_INPUT_BYTES,
            "甲",
            Language::Chinese,
        ),
    ];
    assert!(decode(&input, &edges, ALL).is_some());
    assert!(decode(&(input + "a"), &edges, ALL).is_none());
}

#[test]
fn accepts_the_lexeme_budget_boundary_and_counts_invalid_edges_towards_it() {
    let mut edges = vec![edge(0, 0, "", Language::Chinese); MAX_LEXEMES - 2];
    edges.push(edge(0, 1, "a", Language::English));
    edges.push(edge(1, 2, "甲", Language::Chinese));
    assert!(decode("ab", &edges, ALL).is_some());
    edges.push(edge(0, 0, "", Language::Chinese));
    assert!(decode("ab", &edges, ALL).is_none());
}

#[test]
fn disabling_all_or_all_but_one_language_does_not_create_a_mixed_path() {
    let edges = [
        edge(0, 1, "a", Language::English),
        edge(1, 2, "甲", Language::Chinese),
    ];
    assert!(decode("ab", &edges, &[]).is_none());
    for language in ALL {
        assert!(decode("ab", &edges, &[*language]).is_none());
    }
}

#[test]
fn uses_provider_rank_and_stays_deterministic_when_enumeration_order_changes() {
    let mut preferred = edge(0, 2, "甲", Language::Chinese);
    preferred.rank = 0;
    let mut second = edge(0, 2, "乙", Language::Chinese);
    second.rank = 1;
    let mut edges = vec![second, preferred, edge(2, 5, "abc", Language::English)];
    let forward = decode("abcde", &edges, ALL).expect("正向枚举有完整路径");
    edges.reverse();
    let reversed = decode("abcde", &edges, ALL).expect("反向枚举有完整路径");
    assert_eq!(forward, reversed);
    assert_eq!(text(&forward), "甲abc");
}
