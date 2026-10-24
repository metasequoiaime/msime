use super::{decode, Conversion, Language, Lexeme, Segment};

const ALL: [Language; 3] = [Language::Chinese, Language::English, Language::Japanese];

fn exhaustive(input: &str, lexemes: &[Lexeme], enabled: &[Language]) -> Option<Conversion> {
    // 小图穷举所有完整路径，不复用生产实现的状态裁剪或路径比较器。
    fn visit(
        position: usize,
        end: usize,
        lexemes: &[Lexeme],
        enabled: &[Language],
        path: &mut Vec<usize>,
        complete: &mut Vec<Vec<usize>>,
    ) {
        if position == end {
            complete.push(path.clone());
            return;
        }
        for (index, lexeme) in lexemes.iter().enumerate() {
            if lexeme.input.start == position && enabled.contains(&lexeme.language) {
                path.push(index);
                visit(lexeme.input.end, end, lexemes, enabled, path, complete);
                path.pop();
            }
        }
    }

    let mut paths = Vec::new();
    visit(
        0,
        input.len(),
        lexemes,
        enabled,
        &mut Vec::new(),
        &mut paths,
    );
    let key = |path: &Vec<usize>| {
        let switches = path
            .windows(2)
            .filter(|pair| lexemes[pair[0]].language != lexemes[pair[1]].language)
            .count();
        let rank: usize = path
            .iter()
            .map(|&index| usize::from(lexemes[index].rank))
            .sum();
        let spelling: Vec<_> = path
            .iter()
            .map(|&index| {
                let lexeme = &lexemes[index];
                (
                    lexeme.language,
                    lexeme.text.as_str(),
                    lexeme.input.start,
                    lexeme.input.end,
                )
            })
            .collect();
        (path.len(), switches, rank, spelling)
    };
    paths.sort_by_key(key);
    let is_single = |path: &&Vec<usize>| {
        path.iter()
            .all(|&index| lexemes[index].language == lexemes[path[0]].language)
    };
    let path = paths.iter().find(|path| !is_single(path))?;
    if paths
        .iter()
        .filter(is_single)
        .any(|single| single.len() < path.len())
    {
        return None;
    }
    let chinese_complete = paths.iter().any(|path| {
        path.iter()
            .all(|&index| lexemes[index].language == Language::Chinese)
    });
    let mut segments: Vec<Segment> = Vec::new();
    for &index in path {
        let lexeme = &lexemes[index];
        match segments.last_mut() {
            Some(segment) if segment.language == lexeme.language => {
                segment.input.end = lexeme.input.end;
                segment.text.push_str(&lexeme.text);
            }
            _ => segments.push(Segment {
                input: lexeme.input.clone(),
                language: lexeme.language,
                text: lexeme.text.clone(),
            }),
        }
    }
    Some(Conversion {
        segments,
        chinese_complete,
    })
}

#[test]
fn bounded_search_matches_exhaustive_paths_for_all_language_subsets() {
    let mut seed = 7_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed >> 32
    };
    let mut mixed_cases = 0;
    let mut single_or_uncovered_cases = 0;
    for case in 0..256 {
        let input = "abcdef";
        let mut lexemes = Vec::new();
        for start in 0..input.len() {
            for end in start + 1..=(start + 3).min(input.len()) {
                for language in ALL {
                    if next() % 3 == 0 {
                        lexemes.push(Lexeme {
                            input: start..end,
                            text: format!("{language:?}{start}{end}"),
                            language,
                            rank: (next() % 4) as u16,
                        });
                    }
                }
            }
        }
        for mask in 0_u8..8 {
            let enabled: Vec<_> = ALL
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, language)| (mask & (1 << index) != 0).then_some(language))
                .collect();
            let expected = exhaustive(input, &lexemes, &enabled);
            mixed_cases += usize::from(expected.is_some());
            single_or_uncovered_cases += usize::from(expected.is_none());
            assert_eq!(
                decode(input, &lexemes, &enabled),
                expected,
                "图 {case}，语言 {mask}"
            );
            lexemes.reverse();
            assert_eq!(
                decode(input, &lexemes, &enabled),
                expected,
                "反序图 {case}，语言 {mask}"
            );
        }
    }
    assert!(mixed_cases > 0, "合成小图必须覆盖实际混合路径");
    assert!(
        single_or_uncovered_cases > 0,
        "合成小图必须覆盖不应给混合候选的情况"
    );
}
