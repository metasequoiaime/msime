use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::*;

#[test]
fn reused_output_matches_cold_queries_and_keeps_storage_across_limits() {
    let surfaces: Vec<_> = (0..70).map(|index| format!("合成語{index:02}")).collect();
    let mut entries = vec![
        ("か", "仮", 0, 1, -20),
        ("かか", "仮仮", 1, 0, -100),
        ("かき", "仮木", 0, 0, 30),
        ("かきごおり", "仮氷", 1, 1, 40),
        ("かな", "仮名", 0, 0, 10),
        ("し", "仮", 1, 0, 20),
        ("しし", "仮指", 0, 1, -30),
    ];
    entries.extend(
        surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| ("か", surface.as_str(), 0, 1, index as i32 % 7 - 50)),
    );
    entries.sort_by_key(|entry| entry.0);
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -10, 30, 5]).into_boxed_slice(),
    )
    .expect("合成词库");
    let mut conversions: Vec<_> = ["", "q", "k", "ka", "kak", "kana", "kanasi", "sis", "kaあ"]
        .into_iter()
        .map(convert_romaji)
        .collect();
    conversions.push(convert_romaji(&format!("{}k", "ka".repeat(24))));
    conversions.push(RomajiConversion {
        hiragana: "😀未".repeat(32),
        pending: String::new(),
        complete: true,
    });
    let mut destination = Vec::with_capacity(128);
    let pointer = destination.as_ptr();
    let capacity = destination.capacity();
    for limit in [0, 1, 2, 12, 0, 24, 64, 128, 2] {
        for conversion in &conversions {
            // 留下旧文本与成本，下一轮必须先清空，且零限额也不能留下旧结果。
            destination.push(JapaneseConversion {
                text: "旧合成结果".to_owned(),
                cost: -99_999,
            });
            let (expected, cold_allocations) =
                crate::ime::personal_rerank::allocations::count(|| {
                    search_converted(&dictionary, conversion, limit)
                });
            let ((), reused_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                search_converted_into(&dictionary, conversion, limit, &mut destination);
            });
            assert_eq!(destination, expected, "limit={limit}");
            assert_eq!(destination.as_ptr(), pointer);
            assert_eq!(destination.capacity(), capacity);
            assert_eq!(
                reused_allocations + usize::from(limit > 0),
                cold_allocations,
                "只省结果容器：limit={limit}，读音字节数={}",
                conversion.hiragana.len()
            );
        }
    }
}

#[test]
fn output_buffer_grows_when_needed_and_reuses_the_new_capacity() {
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(&[("か", "仮", 0, 0, -10)], 1, &[0]).into_boxed_slice(),
    )
    .expect("合成词库");
    let conversion = convert_romaji("ka");
    let mut destination = Vec::new();
    for limit in [1, 12, 24, 0, 2, 64] {
        search_converted_into(&dictionary, &conversion, limit, &mut destination);
        assert_eq!(
            destination,
            search_converted(&dictionary, &conversion, limit)
        );
        assert!(destination.capacity() >= limit);
        let pointer = destination.as_ptr();
        let capacity = destination.capacity();
        search_converted_into(&dictionary, &conversion, limit, &mut destination);
        assert_eq!(destination.as_ptr(), pointer);
        assert_eq!(destination.capacity(), capacity);
    }
}
