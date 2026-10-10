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
