use super::tests::{sorted, write_table};
use super::*;

#[test]
fn lookup_preserves_first_match_bits_at_bucket_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("buckets.bin");
    for width in [
        0, 1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
    ] {
        let mut entries = Vec::new();
        for bucket in [0u64, 1, 23, 32768, 65534, 65535] {
            for offset in 0..width {
                let key = (bucket << INDEX_SHIFT) + offset * 131 + 3;
                entries.push((key, offset as f32 - 12.5));
                if offset % 3 == 0 {
                    entries.push((key, 999.0));
                }
            }
        }
        // 极值与特殊浮点位模式仍按落盘值返回，重复键取第一项。
        if width != 0 {
            entries.extend([
                (0, f32::from_bits(0x8000_0000)),
                (0, 123.0),
                (u64::MAX, f32::from_bits(0x7fc0_1234)),
            ]);
        }
        let entries = sorted(entries);
        write_table(&path, &entries, MAGIC, VERSION);
        let table = NgramTable::load(&path).unwrap();
        let mut queries = vec![0, 1, u64::MAX - 1, u64::MAX];
        for bucket in [0u64, 1, 2, 23, 24, 32768, 32769, 65534, 65535] {
            let start = bucket << INDEX_SHIFT;
            queries.extend([start, start | ((1u64 << INDEX_SHIFT) - 1)]);
        }
        for &(key, _) in &entries {
            queries.extend([key.saturating_sub(1), key, key.saturating_add(1)]);
        }
        // 独立线性扫描只认完整键，不借用桶索引或二分逻辑。
        let expected: Vec<_> = queries
            .iter()
            .map(|key| {
                entries
                    .iter()
                    .find(|entry| entry.0 == *key)
                    .map_or(0.0, |entry| entry.1)
                    .to_bits()
            })
            .collect();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            for (&key, &bits) in queries.iter().zip(&expected) {
                assert_eq!(table.score(key).to_bits(), bits, "宽度 {width}，键 {key}");
            }
        });
        assert_eq!(allocations, 0);
    }
}

// 固定远端 e6ec6853f338a38ea472fad90e60ed394cc4e2c6 的完整旧查找正文。
#[inline(never)]
fn reference_lookup(table: &NgramTable, key: u64) -> f32 {
    let bucket = (key >> INDEX_SHIFT) as usize;
    let (mut low, mut high) = (
        table.index[bucket] as usize,
        table.index[bucket + 1] as usize,
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if table.key_at(middle) < key {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if low < table.count && table.key_at(low) == key {
        table.value_at(low)
    } else {
        0.0
    }
}

#[inline(never)]
fn current_lookup(table: &NgramTable, key: u64) -> f32 {
    table.score(key)
}

#[inline(never)]
fn lookup_batch(table: &NgramTable, keys: &[u64], lookup: fn(&NgramTable, u64) -> f32) -> u64 {
    use std::hint::black_box;

    let mut sum = 0u64;
    for _ in 0..4 {
        for &key in keys {
            sum = sum.wrapping_add(u64::from(
                black_box(lookup(black_box(table), black_box(key))).to_bits(),
            ));
        }
    }
    black_box(sum)
}

// 调试构建仍需编译此忽略测试，只在手工运行时拒绝调试计时。
#[allow(clippy::assertions_on_constants)]
#[test]
#[ignore = "需固定 n 元资源与独立计时；设置 MSIME_NEURAL_MODEL_DIR 后手工运行 release"]
fn bucket_lookup_release_budget() {
    use std::time::Instant;

    assert!(!cfg!(debug_assertions), "必须使用 --release 计时");
    let root = std::env::var_os("MSIME_NEURAL_MODEL_DIR").expect("需指定固定资源目录");
    for file in ["msime-bigram.bin", "msime-trigram.bin"] {
        let table = NgramTable::load(&Path::new(&root).join(file)).unwrap();
        assert!(table.count > 0);
        for row in 0..table.count {
            let key = table.key_at(row);
            for query in [key.saturating_sub(1), key, key.saturating_add(1)] {
                assert_eq!(
                    current_lookup(&table, query).to_bits(),
                    reference_lookup(&table, query).to_bits(),
                    "{file} 第 {row} 行的相邻查询 {query}"
                );
            }
        }
        let mut state = 123456789u64;
        let mut hits = Vec::new();
        let mut random = Vec::new();
        let mut ends = Vec::new();
        let mut mixed = Vec::new();
        for index in 0..8192 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let hit = table.key_at(state as usize % table.count);
            hits.push(hit);
            random.push(state);
            ends.push(state | ((1u64 << INDEX_SHIFT) - 1));
            mixed.push(if index % 2 == 0 { hit } else { state });
        }
        for (shape, keys) in [
            ("hit", hits),
            ("random", random),
            ("bucket-end", ends),
            ("mixed", mixed),
        ] {
            for &key in &keys {
                assert_eq!(
                    current_lookup(&table, key).to_bits(),
                    reference_lookup(&table, key).to_bits()
                );
            }
            let expected = lookup_batch(&table, &keys, reference_lookup);
            assert_eq!(lookup_batch(&table, &keys, current_lookup), expected);
            let mut ratios = Vec::with_capacity(80);
            for pair in 0..80 {
                let measure = |lookup| {
                    let start = Instant::now();
                    assert_eq!(lookup_batch(&table, &keys, lookup), expected);
                    start.elapsed().as_nanos() as f64
                };
                let (before, after) = if pair % 2 == 0 {
                    let before = measure(reference_lookup);
                    (before, measure(current_lookup))
                } else {
                    let after = measure(current_lookup);
                    (measure(reference_lookup), after)
                };
                ratios.push(after / before);
            }
            ratios.sort_by(f64::total_cmp);
            eprintln!("{file} {shape} 新/旧配对中位比 {:.4}", ratios[40]);
            assert!(ratios[40] < 0.9, "{file} {shape} 未达到配对预算");
        }
    }
}

// 固定远端 653becfd13baff94bdeed8f9ef22c9a9a6c2a64f 的完整生产查找；
// 上面的 reference_lookup 是更早的字节探针实现，不能作为本次实验的性能基线。
#[inline(never)]
fn partition_reference_lookup(this: &NgramTable, key: u64) -> f32 {
    let bucket = (key >> INDEX_SHIFT) as usize;
    let (start, end) = (this.index[bucket] as usize, this.index[bucket + 1] as usize);
    let (keys, _) = this.bytes[HEADER_BYTES + start * 8..HEADER_BYTES + end * 8].as_chunks::<8>();
    let position = keys.partition_point(|bytes| u64::from_le_bytes(*bytes) < key);
    if keys
        .get(position)
        .is_some_and(|bytes| u64::from_le_bytes(*bytes) == key)
    {
        this.value_at(start + position)
    } else {
        0.0
    }
}

#[inline(never)]
fn strict_budget_batch(
    table: &NgramTable,
    keys: &[u64],
    repeats: usize,
    lookup: fn(&NgramTable, u64) -> f32,
) -> u64 {
    use std::hint::black_box;

    let lookup = black_box(lookup);
    let mut checksum = 0u64;
    for _ in 0..repeats {
        for &key in keys {
            checksum = checksum.wrapping_add(u64::from(
                black_box(lookup(black_box(table), black_box(key))).to_bits(),
            ));
        }
    }
    black_box(checksum)
}

// 旧版先跑同一预算，确认不能用旧臂对自身的噪声跨过 10% 门槛。
// 全部形状输出后才判断失败，避免只保留首个失败或通过的形状。
#[allow(clippy::assertions_on_constants)]
#[test]
#[ignore = "固定完整资源、release、无并行构建时手工运行，必须另存进程监测"]
fn strict_bisect_release_budget() {
    use std::time::{Duration, Instant};

    const PAIRS: usize = 21;
    const MINIMUM: Duration = Duration::from_millis(20);
    const CALIBRATION: Duration = Duration::from_millis(40);

    assert!(!cfg!(debug_assertions), "必须使用 --release 计时");
    let root = std::env::var_os("MSIME_NEURAL_MODEL_DIR").expect("需指定固定资源目录");
    let mut failures = Vec::new();
    for file in ["msime-bigram.bin", "msime-trigram.bin"] {
        let table = NgramTable::load(&Path::new(&root).join(file)).unwrap();
        assert!(table.count > 0);
        // 逐行和相邻键的评分对照在计时外完成，保留完整位模式。
        for row in 0..table.count {
            let key = table.key_at(row);
            for query in [key.saturating_sub(1), key, key.saturating_add(1)] {
                assert_eq!(
                    current_lookup(&table, query).to_bits(),
                    partition_reference_lookup(&table, query).to_bits(),
                    "{file} 第 {row} 行，相邻查询 {query}"
                );
            }
        }
        let mut state = 123456789u64;
        // 同时检查所有高位桶的尾部及均匀伪随机查询，覆盖空桶和末桶。
        for _ in 0..65536 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            for key in [state, state | ((1u64 << INDEX_SHIFT) - 1)] {
                assert_eq!(
                    current_lookup(&table, key).to_bits(),
                    partition_reference_lookup(&table, key).to_bits(),
                    "{file} 随机或桶尾查询 {key}"
                );
            }
        }
        for bucket in 0..(1u64 << INDEX_BITS) {
            let key = (bucket << INDEX_SHIFT) | ((1u64 << INDEX_SHIFT) - 1);
            assert_eq!(
                current_lookup(&table, key).to_bits(),
                partition_reference_lookup(&table, key).to_bits(),
                "{file} 桶 {bucket} 尾部"
            );
        }

        let mut hits = Vec::new();
        let mut random = Vec::new();
        let mut tails = Vec::new();
        let mut mixed = Vec::new();
        for index in 0..8192 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let hit = table.key_at((state % table.count as u64) as usize);
            hits.push(hit);
            random.push(state);
            tails.push(state | ((1u64 << INDEX_SHIFT) - 1));
            mixed.push(if index % 2 == 0 { hit } else { state });
        }
        for (shape, keys) in [
            ("hit", hits),
            ("random", random),
            ("bucket-tail", tails),
            ("mixed", mixed),
        ] {
            for &key in &keys {
                assert_eq!(
                    current_lookup(&table, key).to_bits(),
                    partition_reference_lookup(&table, key).to_bits()
                );
            }
            let checksum = strict_budget_batch(&table, &keys, 1, partition_reference_lookup);
            let measure = |repeats, lookup| {
                let start = Instant::now();
                let actual = strict_budget_batch(&table, &keys, repeats, lookup);
                let duration = start.elapsed();
                assert_eq!(actual, checksum.wrapping_mul(repeats as u64));
                duration
            };
            let mut repeats = 1usize;
            loop {
                let before = measure(repeats, partition_reference_lookup);
                let after = measure(repeats, current_lookup);
                if before >= CALIBRATION && after >= CALIBRATION {
                    break;
                }
                repeats *= 2;
                assert!(repeats <= 1 << 20, "{file} {shape} 校准次数超限");
            }
            let mut ratios = Vec::with_capacity(PAIRS);
            let mut samples = Vec::with_capacity(PAIRS);
            let mut long_enough = true;
            for pair in 0..PAIRS {
                let (before, after) = if pair % 2 == 0 {
                    let before = measure(repeats, partition_reference_lookup);
                    (before, measure(repeats, current_lookup))
                } else {
                    let after = measure(repeats, current_lookup);
                    (measure(repeats, partition_reference_lookup), after)
                };
                long_enough &= before >= MINIMUM && after >= MINIMUM;
                let ratio = after.as_secs_f64() / before.as_secs_f64();
                ratios.push(ratio);
                samples.push(serde_json::json!({
                    "pair": pair,
                    "old_first": pair % 2 == 0,
                    "before_ns": before.as_nanos() as u64,
                    "after_ns": after.as_nanos() as u64,
                    "ratio": ratio,
                }));
            }
            ratios.sort_by(f64::total_cmp);
            let median = ratios[PAIRS / 2];
            eprintln!(
                "strict-bisect-budget {}",
                serde_json::json!({
                    "file": file,
                    "shape": shape,
                    "queries": keys.len(),
                    "repeats": repeats,
                    "median": median,
                    "long_enough": long_enough,
                    "samples": samples,
                })
            );
            if !long_enough || median >= 0.90 {
                failures.push(format!(
                    "{file} {shape}: 中位比 {median:.6}，批次足够长={long_enough}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "严格二分预算失败：{}",
        failures.join("；")
    );
}
