use super::test_model;
use super::*;

#[test]
fn exact_stream_preserves_all_fields_limits_ties_and_one_saved_result_vector() {
    let surfaces: Vec<_> = (0..70).map(|index| format!("合成{index:02}")).collect();
    let mut entries: Vec<_> = surfaces
        .iter()
        .enumerate()
        .map(|(index, surface)| {
            (
                "かな",
                surface.as_str(),
                (index % 2) as u16,
                (index % 2) as u16,
                index as i32 % 7 - 3,
            )
        })
        .collect();
    entries.extend([("き", "合成木", 0, 0, 0), ("甲😀かな", "合成長", 1, 0, -50)]);
    entries.sort_by_key(|entry| entry.0);
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -20, 80, 10]).into_boxed_slice(),
    )
    .expect("合成词库");
    for reading in ["", "か", "かな", "き", "く", "甲😀かな", &"😀".repeat(128)] {
        for limit in [0, 1, 2, 24, 64, 65, 70, 100] {
            let (expected, old_allocations) =
                crate::ime::personal_rerank::allocations::count(|| {
                    dictionary.exact_lemma_views(reading, limit)
                });
            let mut actual = Vec::with_capacity(expected.len());
            let ((), new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.for_each_exact_lemma_view(reading, limit, |view| actual.push(view))
            });
            assert_eq!(actual, expected, "读音={reading}，限额={limit}");
            assert_eq!(
                new_allocations + usize::from(!expected.is_empty()),
                old_allocations
            );
            if reading == "かな" && limit > 0 {
                let mut ids: Vec<u32> = (0..70).collect();
                ids.sort_unstable_by_key(|id| (id % 7, *id));
                assert_eq!(
                    actual.iter().map(|view| view.token_id).collect::<Vec<_>>(),
                    ids[..limit.min(ids.len())]
                );
            }
        }
    }
    let mut saved = None;
    {
        // 查询文本释放后，视图仍只借用词库。
        let query = String::from("甲😀かな");
        dictionary.for_each_exact_lemma_view(&query, 1, |view| saved = Some(view));
    }
    assert_eq!(saved.unwrap().surface, "合成長");
    assert_eq!(saved.unwrap().reading, "甲😀かな");
}

#[test]
fn reusing_exact_stream_matches_ranked_order_and_reuses_its_buffer() {
    let surfaces: Vec<_> = (0..300).map(|index| format!("合成{index:03}")).collect();
    let mut entries: Vec<_> = surfaces
        .iter()
        .enumerate()
        .map(|(index, surface)| ("かな", surface.as_str(), 0, 0, index as i32 % 11 - 5))
        .collect();
    entries.extend([("かなし", "合成悲", 0, 0, -90), ("き", "合成木", 0, 0, 0)]);
    entries.sort_by_key(|entry| entry.0);
    let dictionary =
        JapaneseDictionary::from_bytes(test_model::bytes(&entries, 1, &[0]).into_boxed_slice())
            .expect("合成词库");
    let mut ranking = Vec::new();
    for reading in ["", "か", "かな", "かなし", "き", "く"] {
        for limit in [0, 1, 24, 299, 300, 512] {
            let expected = dictionary.exact_lemma_views(reading, limit);
            let mut actual = Vec::new();
            dictionary.for_each_exact_lemma_view_reusing(reading, limit, &mut ranking, |view| {
                actual.push(view)
            });
            assert_eq!(actual, expected, "读音={reading}，限额={limit}");
        }
    }
    // 缓冲已容下整组，再查同一读音不申请。
    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        dictionary.for_each_exact_lemma_view_reusing("かな", 512, &mut ranking, |_| {})
    });
    assert_eq!(allocations, 0);
}
