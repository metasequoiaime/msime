use std::sync::Arc;

use super::super::ngram::tests::pair_table;
use super::*;

#[test]
fn a_memo_miss_hashes_the_key_once() {
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "memo.bin",
        &[("前", "后", 1.0)],
    ));
    let mut memo = BigramMemo::default();
    BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.set(0));

    assert_eq!(memo.score(&table, "前", "后"), 1.0);
    BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.set(0));
    assert_eq!(memo.score(&table, "新前", "新后"), 0.0);

    let hash_calls = BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.get());
    assert_eq!(
        hash_calls, 1,
        "一次新的二元分键不应在缓存查询和插入时重复哈希"
    );
}
