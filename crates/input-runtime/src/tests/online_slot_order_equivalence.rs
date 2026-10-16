use super::*;

/// 对照的每页候选数：5，和偏好默认的 6。
const PAGE_SIZES: [u8; 2] = [5, 6];

// 参考实现按组依次追加下标，避免用生产代码的新座位偏移公式作为预期。AI 行不进分组（#6888）：其余行有云候选时按组排，没有时保持原顺序；第一条 AI 行再插进第一页最后一格（不足一页时接在其余行之后），其余 AI 行接在最后。
fn reference_order(
    sources: &[u8],
    positions: &[u8],
    single_kana: bool,
    page_size: u8,
) -> Vec<usize> {
    if sources.len() < 2 || !sources.iter().any(|source| matches!(source, 2 | 3 | 12)) {
        return (0..sources.len()).collect();
    }
    let group = |source| {
        sources
            .iter()
            .enumerate()
            .filter(|(_, value)| **value == source)
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    };
    let local: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, source)| !matches!(source, 2 | 3 | 4 | 6 | 7 | 12 | 13))
        .map(|(index, _)| index)
        .collect();
    let cloud = group(2);
    let ai: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, source)| matches!(source, 3 | 12))
        .map(|(index, _)| index)
        .collect();
    let mut order: Vec<usize> = if cloud.is_empty() {
        (0..sources.len())
            .filter(|index| !matches!(sources[*index], 3 | 12))
            .collect()
    } else {
        let english = group(4);
        let japanese = group(13);
        let emoji = group(6);
        let kaomoji = group(7);
        let promoted_english = !local.is_empty() && english.first() == Some(&0);
        let promoted_japanese =
            !local.is_empty() && japanese.first() == Some(&usize::from(promoted_english));
        let local_prefix = if single_kana { 2 } else { 1 };
        let mut order = Vec::new();
        if promoted_english {
            order.push(0);
        }
        if promoted_japanese {
            order.extend(japanese.first().copied());
        }
        order.extend(local.iter().take(local_prefix).copied());
        order.extend(&cloud);
        if !promoted_japanese {
            order.extend(japanese.first().copied());
        }
        if !promoted_english {
            order.extend(english.first().copied());
        }
        order.extend(emoji.first().copied());
        order.extend(kaomoji.first().copied());
        order.extend(local.iter().skip(local_prefix).copied());
        for group in [&english, &japanese, &emoji, &kaomoji] {
            order.extend(group.iter().skip(1).copied());
        }
        let mut fixed = Vec::new();
        order.retain(|&index| {
            if sources[index] == 4 && positions[index] > 0 {
                fixed.push(index);
                false
            } else {
                true
            }
        });
        fixed.sort_by_key(|&index| positions[index]);
        for index in fixed {
            order.insert(usize::from(positions[index] - 1).min(order.len()), index);
        }
        order
    };
    if let Some(&first) = ai.first() {
        let seat = usize::from(page_size - 1).max(1).min(order.len());
        order.insert(seat, first);
    }
    order.extend(ai.iter().skip(1).copied());
    order
}

fn check(sources: Vec<u8>, positions: Vec<u8>, single_kana: bool) {
    for page_size in PAGE_SIZES {
        check_page(sources.clone(), positions.clone(), single_kana, page_size);
    }
}

fn check_page(sources: Vec<u8>, positions: Vec<u8>, single_kana: bool, page_size: u8) {
    let rows = sources.len();
    let expected = reference_order(&sources, &positions, single_kana, page_size);
    let mut runtime = Runtime::new(
        Fixture {
            words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
            codes: (0..rows).map(|index| format!("k{index}")).collect(),
            sources: sources.clone(),
            positions: positions.clone(),
            text: "a".into(),
            scheme: if single_kana {
                msime_engine::SchemeType::JapaneseRomaji as u8
            } else {
                0
            },
            reading: if single_kana {
                "あ".into()
            } else {
                String::new()
            },
            ..Fixture::default()
        },
        page_size,
    )
    .unwrap();
    runtime.refresh().unwrap();
    for (seat, &index) in expected.iter().enumerate() {
        assert_eq!(runtime.cached.candidates[seat], format!("合成候选{index}"));
        assert_eq!(runtime.cached.candidate_codes[seat], format!("k{index}"));
        assert_eq!(
            runtime.cached.candidate_annotations[seat],
            format!("({index})")
        );
        assert_eq!(runtime.cached.candidate_sources[seat], sources[index]);
        assert_eq!(runtime.cached.candidate_positions[seat], positions[index]);
        assert!(!runtime.cached.candidate_corrected[seat]);
        assert!(runtime.cached.candidate_answers_key[seat]);
    }
    assert_eq!(runtime.cached.candidates.len(), expected.len());
    if expected
        .iter()
        .enumerate()
        .all(|(seat, &index)| seat == index)
    {
        assert!(runtime.engine_order.is_empty());
    } else {
        assert_eq!(runtime.engine_order, expected);
    }
}

#[test]
fn online_slots_match_group_append_rules_across_sources_and_fixed_positions() {
    const SOURCES: [u8; 9] = [0, 2, 3, 4, 6, 7, 12, 13, 14];
    for rows in 0..=4 {
        for mut value in 0..SOURCES.len().pow(rows as u32) {
            let sources: Vec<u8> = (0..rows)
                .map(|_| {
                    let source = SOURCES[value % SOURCES.len()];
                    value /= SOURCES.len();
                    source
                })
                .collect();
            for mode in 0..3 {
                let positions = (0..rows)
                    .map(|index| match mode {
                        0 => 0,
                        1 => 2,
                        _ => [1, 2, 255][index % 3],
                    })
                    .collect::<Vec<u8>>();
                for single_kana in [false, true] {
                    check(sources.clone(), positions.clone(), single_kana);
                }
            }
        }
    }
    let mut state = 0x7541_9812_5612_3819_u64;
    for rows in [8, 32, 128, 129, 1024] {
        for _ in 0..64 {
            let sources: Vec<u8> = (0..rows)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    SOURCES[(state >> 32) as usize % SOURCES.len()]
                })
                .collect();
            let positions = (0..rows)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    [0, 1, 2, 3, 255][(state >> 32) as usize % 5]
                })
                .collect::<Vec<u8>>();
            for single_kana in [false, true] {
                check(sources.clone(), positions.clone(), single_kana);
            }
        }
    }
}
