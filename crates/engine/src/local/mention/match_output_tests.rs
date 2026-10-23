use super::*;
use crate::ime::personal_rerank::allocations;

fn assert_output_budget(code: &str, entries: &[MentionEntry], with_places: bool) {
    let expected = frozen_query_mentions(code, entries, with_places);
    // 每个非空字段是一个字符串加一个 `SharedString` 的 `Arc`；空字段不占存储。
    let output_allocations = usize::from(!expected.is_empty())
        + expected
            .iter()
            .map(|row| {
                2 * (usize::from(!row.pinyin.is_empty()) + usize::from(!row.word.is_empty()))
            })
            .sum::<usize>();
    let (actual, allocations) = allocations::count(|| query_mentions(code, entries, with_places));
    assert_eq!(actual, expected, "code={code:?}, with_places={with_places}");
    assert_eq!(actual.capacity(), actual.len());
    assert_eq!(
        allocations, output_allocations,
        "@ 查询只应申请最终候选的向量和字符串：code={code:?}, with_places={with_places}"
    );
}

fn entry(text: &str, key: &str) -> MentionEntry {
    MentionEntry {
        phone: String::new(),
        text: text.to_owned(),
        key: key.to_owned(),
    }
}

#[test]
fn host_queries_allocate_only_owned_output() {
    let entries = usable_mentions(&[
        entry("前缀", "zhang'shan'shan"),
        entry("精确", "zhang'san"),
        entry("深圳市", "shen'zhen'shi"),
        entry("Alice", ""),
    ]);
    for code in ["", "zs", "zhangsan", "al", "zzzz"] {
        assert_output_budget(code, &entries, false);
    }
}

#[test]
fn mixed_place_queries_allocate_only_owned_output() {
    places();
    let entries = usable_mentions(&[
        entry("用户前缀", "zhang'shan'shan"),
        entry("用户精确", "zhang'san"),
        entry("深圳市", "shen'zhen'shi"),
    ]);
    for code in ["zh", "zs", "bj", "bei", "shenzhen", "", "zzzz"] {
        assert_output_budget(code, &entries, true);
        assert_output_budget(code, &[], true);
    }
}

#[test]
fn fixed_match_outputs_preserve_frozen_query_boundaries() {
    places();
    for count in [
        0,
        1,
        RESULT_LIMIT - 1,
        RESULT_LIMIT,
        RESULT_LIMIT + 1,
        LIST_LIMIT,
    ] {
        let entries: Vec<_> = (0..count)
            .map(|index| {
                let key = match index % 3 {
                    0 => "b'j",
                    1 => "bei'jing'shi",
                    _ => "z's",
                };
                entry(&format!("合成{index}"), key)
            })
            .collect();
        let entries = usable_mentions(&entries);
        for code in ["", "b", "bj", "beijing", "z", "zs", "luan", "zzzz"] {
            for with_places in [false, true] {
                assert_output_budget(code, &entries, with_places);
            }
        }
    }
}

#[test]
fn place_match_limits_and_dedup_preserve_frozen_output() {
    let table = places();
    for code in ["", "b", "bj", "bei", "shenzhen", "luanshi", "zzzz"] {
        for limit in [
            0,
            1,
            2,
            RESULT_LIMIT - 1,
            RESULT_LIMIT,
            RESULT_LIMIT + 1,
            usize::MAX,
        ] {
            for existing in [
                &[][..],
                &["北京市"][..],
                &["北京市", "深圳市", "朝阳区"][..],
            ] {
                let expected = frozen_place_matches(table, code, existing, limit);
                let ((actual, count), allocations) =
                    allocations::count(|| collect_place_matches(table, code, existing, limit));
                assert_eq!(&actual[..count], expected);
                assert!(count <= limit.min(RESULT_LIMIT));
                assert_eq!(allocations, 0);
            }
        }
    }
}

#[test]
fn mixed_place_output_reduces_transient_heap_without_changing_retained_storage() {
    places();
    let entries = usable_mentions(&[
        entry("用户前缀", "zhang'shan'shan"),
        entry("用户精确", "zhang'san"),
    ]);
    let (expected, old) = allocations::measure(|| frozen_query_mentions("zh", &entries, true));
    let (actual, new) = allocations::measure(|| query_mentions("zh", &entries, true));
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), RESULT_LIMIT);
    assert_eq!(new.allocations + 3, old.allocations);
    assert_eq!(new.remaining_bytes, old.remaining_bytes);
    assert_eq!(new.minimum_bytes, 0);
    assert_eq!(old.minimum_bytes, 0);
    assert_eq!(new.peak_bytes as i128, new.remaining_bytes);
    assert!(old.peak_bytes >= new.peak_bytes + RESULT_LIMIT * std::mem::size_of::<(&str, &str)>());
}

// 冻结优化前的实现，分别保留两层堆上匹配行作为字段、顺序和分配预算的对照。
fn frozen_place_matches(
    table: &Places,
    code: &str,
    existing_names: &[&str],
    limit: usize,
) -> Vec<(&'static str, &'static str)> {
    let limit = limit.min(RESULT_LIMIT);
    if limit == 0 {
        return Vec::new();
    }
    let mut exact = [None; RESULT_LIMIT];
    let mut exact_len = 0;
    let mut prefix = [None; RESULT_LIMIT];
    let mut prefix_len = 0;
    let mut matched_names = [None; PLACE_MATCH_CAPACITY];
    let mut matched_names_len = 0;
    for (place, spellings) in table.places.iter().zip(&table.spellings) {
        let Some(is_exact) = spelling_match(spellings, code) else {
            continue;
        };
        if existing_names.contains(&place.name)
            || matched_names[..matched_names_len]
                .iter()
                .flatten()
                .any(|name| *name == place.name)
        {
            continue;
        }
        if is_exact {
            if exact_len == limit {
                continue;
            }
            exact[exact_len] = Some((place.key, place.name));
            exact_len += 1;
        } else if prefix_len < limit {
            prefix[prefix_len] = Some((place.key, place.name));
            prefix_len += 1;
        } else {
            continue;
        }
        matched_names[matched_names_len] = Some(place.name);
        matched_names_len += 1;
        if exact_len == limit {
            break;
        }
    }
    let prefix_len = prefix_len.min(limit.saturating_sub(exact_len));
    let mut matches = Vec::with_capacity(exact_len + prefix_len);
    for place in exact[..exact_len].iter().chain(prefix[..prefix_len].iter()) {
        matches.push(place.expect("place row slot is filled"));
    }
    matches
}

fn frozen_query_mentions(code: &str, entries: &[MentionEntry], with_places: bool) -> Vec<WordItem> {
    let (exact, exact_len, prefix, prefix_len) = collect_mention_rows(code, entries);
    let prefix_len = prefix_len.min(RESULT_LIMIT - exact_len);
    let mut matches = Vec::with_capacity(exact_len + prefix_len);
    for entry in exact[..exact_len].iter().chain(prefix[..prefix_len].iter()) {
        let entry = entry.expect("mention row slot is filled");
        matches.push((entry.key.as_str(), entry.text.as_str()));
    }
    if with_places && !code.is_empty() {
        let table = places();
        let existing_len = matches.len();
        let mut existing_names = [""; RESULT_LIMIT];
        for (index, (_, text)) in matches.iter().enumerate() {
            existing_names[index] = text;
        }
        matches.extend(frozen_place_matches(
            table,
            code,
            &existing_names[..existing_len],
            RESULT_LIMIT.saturating_sub(matches.len()),
        ));
    }
    let count = matches.len();
    matches
        .into_iter()
        .enumerate()
        .map(|(index, (key, text))| {
            WordItem::new(
                key,
                text,
                (count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}
