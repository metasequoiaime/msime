//! 九键精确模式下，已命中长读法后仍保留可继续输入的尾段补全。

use super::*;

const GRID_TAIL_COMPLETION_LIMIT: usize = 12;

impl PinyinDatabase {
    pub(crate) fn query_grid_tail_completions(
        &self,
        segments: &[String],
        key: &str,
    ) -> Vec<DictRow> {
        if segments.len() < 4 || !has_only_complete_pinyin_segments(segments) {
            return Vec::new();
        }
        let Some(table) = build_table_name(segments) else {
            return Vec::new();
        };
        let bound = sql_limit(GRID_TAIL_COMPLETION_LIMIT);
        let sql = format!(
            "SELECT \"key\", \"value\", \"weight\" FROM \"{table}\" WHERE \"key\" > ?1 AND \"key\" < ?2 ORDER BY \"weight\" DESC LIMIT {bound}"
        );
        let upper = key_prefix_upper_bound(key);
        self.rows(
            &sql,
            [key, upper.as_str()],
            query_capacity(GRID_TAIL_COMPLETION_LIMIT),
        )
    }
}
