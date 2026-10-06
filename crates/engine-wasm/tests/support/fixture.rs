//! 最小的 msime.db：表结构和发布的主库一致（`tbl_<长度>_<首字母>` 加 `idx_key_*`/`idx_jp_*` 索引，`wubi86`/`wubi98`/`quick_parases` 加各自的索引），行只有路由测试用到的那些。`make_fixture` 示例和 `tests/routing.rs` 共用这一份。

use std::path::Path;

use rusqlite::Connection;

/// 主库里拼音表的首字母（没有 i、u、v 开头的音节）。
const INITIALS: &str = "abcdefghjklmnopqrstwxyz";

/// 拼音表的长度分组：1 到 7 个音节，以及更长的 `others`。
const LENGTHS: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "others"];

/// 单音节 `a` 的行数多于引擎首次查询给出的 24 个，翻到第 4 页（下标 3）之前必须先展开。
const A_ROWS: [&str; 30] = [
    "啊", "阿", "吖", "腌", "锕", "嗄", "厑", "呵", "錒", "婀", "屙", "痾", "哎", "安", "庵", "鞍",
    "氨", "桉", "谙", "埯", "揞", "铵", "鹌", "盦", "侒", "峖", "垵", "痷", "媕", "闇",
];

/// 拼音行：表名、key、简拼、字、权重。
const PINYIN_ROWS: [(&str, &str, &str, &str, i64); 15] = [
    ("tbl_2_n", "ni'hao", "nh", "你好", 332_885),
    ("tbl_2_n", "ni'hao", "nh", "拟好", 3_685),
    ("tbl_1_n", "ni", "n", "你", 9_000_000),
    ("tbl_1_n", "ni", "n", "呢", 4_000_000),
    ("tbl_1_n", "ni", "n", "尼", 900_000),
    ("tbl_1_h", "hao", "h", "好", 9_000_000),
    ("tbl_1_h", "hao", "h", "号", 3_000_000),
    ("tbl_1_h", "hao", "h", "毫", 400_000),
    ("tbl_2_x", "xi'an", "xa", "西安", 55_003),
    ("tbl_1_x", "xian", "x", "先", 5_000_000),
    ("tbl_1_x", "xian", "x", "现", 4_000_000),
    ("tbl_1_x", "xi", "x", "西", 3_000_000),
    ("tbl_1_x", "xi", "x", "系", 2_000_000),
    ("tbl_1_a", "an", "a", "按", 2_500_000),
    ("tbl_1_a", "an", "a", "暗", 1_500_000),
];

/// 五笔 86 行：`wqvb` 只有一行（四码唯一，自动上屏）；`ggll` 有两行（满四码后第五个字母顶字）。码表里没有任何以 `x` 开头的码，`xyxy` 是空码。
const WUBI_ROWS: [(&str, &str, i64); 3] = [
    ("wqvb", "你好", 1_000),
    ("ggll", "五一", 900),
    ("ggll", "一五", 800),
];

fn code_table(name: &str) -> String {
    format!(
        "CREATE TABLE {name} (\"key\" TEXT NOT NULL, \"value\" TEXT NOT NULL, \"weight\" INTEGER NOT NULL DEFAULT 0, UNIQUE(\"key\", \"value\"));\
         CREATE INDEX idx_{name}_key_weight ON {name}(\"key\", \"weight\" DESC);"
    )
}

/// 在 `path` 写出夹具库。`only_wubi86` 时像 M4 的五笔库一样清空所有拼音表（表结构保留），只留 `wubi86` 的行。
pub fn write(path: &Path, only_wubi86: bool) -> rusqlite::Result<()> {
    let mut connection = Connection::open(path)?;
    let transaction = connection.transaction()?;
    let mut schema = String::new();
    for length in LENGTHS {
        for initial in INITIALS.chars() {
            schema.push_str(&format!(
                "CREATE TABLE tbl_{length}_{initial} (\"key\" text, \"jp\" text, \"value\" text, \"weight\" integer default 0);\
                 CREATE INDEX idx_key_{length}_{initial} on tbl_{length}_{initial}(key);\
                 CREATE INDEX idx_jp_{length}_{initial} on tbl_{length}_{initial}(jp);"
            ));
        }
    }
    for table in ["wubi86", "wubi98", "quick_parases"] {
        schema.push_str(&code_table(table));
    }
    transaction.execute_batch(&schema)?;
    if !only_wubi86 {
        let mut insert = transaction
            .prepare("INSERT INTO tbl_1_a (key, jp, value, weight) VALUES ('a', 'a', ?1, ?2)")?;
        for (rank, word) in A_ROWS.iter().enumerate() {
            // 权重各不相同，排序因此是确定的。
            insert.execute((word, 1_000_000 - 1_000 * rank as i64))?;
        }
        drop(insert);
        for (table, key, jp, value, weight) in PINYIN_ROWS {
            transaction.execute(
                &format!("INSERT INTO {table} (key, jp, value, weight) VALUES (?1, ?2, ?3, ?4)"),
                (key, jp, value, weight),
            )?;
        }
    }
    for (key, value, weight) in WUBI_ROWS {
        transaction.execute(
            "INSERT INTO wubi86 (key, value, weight) VALUES (?1, ?2, ?3)",
            (key, value, weight),
        )?;
    }
    transaction.commit()
}
