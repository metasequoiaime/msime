use super::*;

#[test]
fn fixed_prefix_index_matches_original_scan_and_case_variants() {
    let mut prefixes = HashSet::new();
    for &(romaji, _) in ROMAJI_TABLE {
        assert!(romaji.is_ascii());
        assert!(!romaji.bytes().any(|byte| byte.is_ascii_uppercase()));
        for length in 1..=romaji.len() {
            let prefix = &romaji[..length];
            prefixes.insert(prefix.as_bytes());
            for mask in 0..(1 << length) {
                let input: String = prefix
                    .bytes()
                    .enumerate()
                    .map(|(index, byte)| {
                        char::from(if mask & (1 << index) != 0 {
                            byte.to_ascii_uppercase()
                        } else {
                            byte
                        })
                    })
                    .collect();
                let actual = kana_for_romaji_prefix_view(&input);
                assert_eq!(actual, kana_for_romaji_prefix(&input), "{input}");
                assert!(actual.windows(2).all(|pair| pair[0] < pair[1]));
            }
            for suffix in ["?", "é", "あ", " "] {
                let input = format!("{prefix}{suffix}");
                assert_eq!(
                    kana_for_romaji_prefix_view(&input),
                    kana_for_romaji_prefix(&input)
                );
            }
        }
    }
    assert_eq!(
        ROMAJI_PREFIX_KANA.keys().copied().collect::<HashSet<_>>(),
        prefixes
    );
    for input in [
        "",
        "?",
        "あ",
        "É",
        "Kあ",
        "éK",
        "k\0",
        "1234",
        "xxxxxxxxxxxxxxxx",
    ] {
        assert_eq!(
            kana_for_romaji_prefix_view(input),
            kana_for_romaji_prefix(input)
        );
    }
    assert_eq!(
        kana_for_romaji_prefix_view("sh"),
        ["し", "しぇ", "しゃ", "しゅ", "しょ"]
    );
    assert_eq!(kana_for_romaji_prefix_view("xtu"), ["っ"]);
    assert_eq!(kana_for_romaji_prefix_view("zya"), ["じゃ"]);
    assert_eq!(kana_for_romaji_prefix_view("dya"), ["ぢゃ"]);
    assert_eq!(kana_for_romaji_prefix_view("-"), ["ー"]);
}

#[test]
fn prefix_views_outlive_input_and_share_storage_across_threads() {
    let view = {
        let input = String::from("K");
        kana_for_romaji_prefix_view(&input)
    };
    assert!(!view.is_empty());
    assert!(std::ptr::eq(view, kana_for_romaji_prefix_view("k")));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                assert!(std::ptr::eq(view, kana_for_romaji_prefix_view("K")));
            });
        }
    });
}

#[test]
#[ignore = "本地 release 计时，独立进程测冷初始化；不设置 CI 时间阈值"]
fn benchmark_fixed_kana_prefix_index() {
    use std::hint::black_box;
    use std::time::Instant;

    assert!(
        LazyLock::get(&ROMAJI_PREFIX_KANA).is_none(),
        "只单独运行本测试以测量冷初始化"
    );
    let start = Instant::now();
    let (view, cold_allocations) =
        crate::ime::personal_rerank::allocations::count(|| kana_for_romaji_prefix_view("k"));
    let cold_elapsed = start.elapsed();
    assert_eq!(view, kana_for_romaji_prefix("k"));
    let entries = ROMAJI_PREFIX_KANA.len();
    let map_capacity = ROMAJI_PREFIX_KANA.capacity();
    let value_lengths: usize = ROMAJI_PREFIX_KANA.values().map(Vec::len).sum();
    let value_capacity: usize = ROMAJI_PREFIX_KANA.values().map(Vec::capacity).sum();
    // 仅计算值缓冲字节；HashMap 的桶、控制字节和分配器额外开销不包含在内。
    let value_bytes = value_capacity * std::mem::size_of::<&str>();
    eprintln!("固定前缀索引冷构建：{cold_elapsed:?}，分配 {cold_allocations}，键 {entries}，map capacity {map_capacity}，假名长度 {value_lengths}，假名 capacity {value_capacity}，值缓冲 {value_bytes} bytes");

    for input in ["k", "sh", "K", "SH", "?", "あ", "toolong", ""] {
        let expected = kana_for_romaji_prefix(input);
        let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            black_box(kana_for_romaji_prefix(black_box(input)))
        });
        let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            black_box(kana_for_romaji_prefix_view(black_box(input)))
        });
        assert_eq!(kana_for_romaji_prefix_view(input), expected);
        assert_eq!(new_allocations, 0);
        let mut old_batches = Vec::new();
        let mut new_batches = Vec::new();
        for batch in 0..10 {
            let mut run_old = || {
                let start = Instant::now();
                for _ in 0..100_000 {
                    black_box(kana_for_romaji_prefix(black_box(input)));
                }
                old_batches.push(start.elapsed());
            };
            let mut run_new = || {
                let start = Instant::now();
                for _ in 0..100_000 {
                    black_box(kana_for_romaji_prefix_view(black_box(input)));
                }
                new_batches.push(start.elapsed());
            };
            if batch % 2 == 0 {
                run_old();
                run_new();
            } else {
                run_new();
                run_old();
            }
        }
        old_batches.sort_unstable();
        new_batches.sort_unstable();
        eprintln!("前缀 {input:?}：分配 {old_allocations}→{new_allocations}，十批中位批次 {:?}→{:?}/100000次", old_batches[5], new_batches[5]);
    }
}
