//! 双拼词网格的混合声母跨度查询，完整音节仍然精确匹配。

use super::*;

impl PinyinDatabase {
    pub(crate) fn query_lattice_span_with_mixed_initials(
        &self,
        span: &[String],
        limit: usize,
    ) -> Vec<DictRow> {
        if span.iter().all(|part| is_intact(part))
            || span
                .split_last()
                .is_some_and(|(_, leading)| leading.iter().all(|part| is_intact(part)))
        {
            return self.query_lattice_span_with_trailing_initial(span, limit);
        }
        if self.connection.is_none()
            || span.is_empty()
            || limit == 0
            || !span.iter().all(|part| {
                is_intact(part) || (is_prefix(part) && extract_initial_token(part) == part)
            })
        {
            return Vec::new();
        }
        let Some(table) = build_table_name(span) else {
            return Vec::new();
        };
        let jp = segments_to_jianpin(span);
        let scan_limit = sql_limit(build_mixed_jianpin_scan_limit(limit));
        let mut result = Vec::new();
        self.visit_rows_until(&jianpin_sql(&table, scan_limit), [jp.as_str()], |row| {
            let mut actual = row.key.split('\'');
            let matches = span.iter().all(|expected| {
                let Some(syllable) = actual.next() else {
                    return false;
                };
                is_intact(syllable)
                    && if is_intact(expected) {
                        syllable == canonical_lattice_syllable(expected)
                    } else {
                        extract_initial_token(syllable) == expected
                    }
            }) && actual.next().is_none();
            if matches {
                result.push(row);
            }
            result.len() < limit
        });
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn mixed_spans_keep_complete_syllables_and_strict_initials() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("synthetic.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_3_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
             INSERT INTO tbl_3_c VALUES\
             ('che''ke''ju','ckj','车科具',9000),\
             ('ce''ken''ju','ckj','测肯具',8000),\
             ('ce''ke''ju','ckj','测科具',100),\
             ('ci''ke''ju','ckj','次科具',90),\
             ('ce''ju''gong','cjg','测具工',80),\
             ('ce''ai''ju','caj','测爱具',9000),\
             ('ce''a''ju','caj','测阿具',70);",
            )
            .unwrap();
        let database = PinyinDatabase::open(&path);
        let query = |parts: &[&str], limit| {
            database.query_lattice_span_with_mixed_initials(&segments(parts), limit)
        };
        let rows = query(&["c", "ke", "ju"], 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "ce'ke'ju");
        let rows = query(&["c", "ke", "ju"], 2);
        assert_eq!(
            rows.iter()
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["测科具", "次科具"]
        );
        assert_eq!(query(&["c", "jv", "g"], 32)[0].key, "ce'ju'gong");
        assert_eq!(query(&["c", "a", "j"], 32)[0].key, "ce'a'ju");
        assert!(query(&["c", "ke", "ju"], 0).is_empty());
        assert!(query(&["c", "ia", "j"], 32).is_empty());
        assert!(query(&[], 32).is_empty());
    }

    #[test]
    fn mixed_overflow_spans_match_the_exact_number_of_syllables() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("synthetic.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_others_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
             INSERT INTO tbl_others_c VALUES\
             ('ce''ke''ni''ni''ni''ni''ni''ju','cknnnnnj','合成八字',100),\
             ('ce''ke''ni''ni''ni''ni''ni''ju''ji','cknnnnnj','合成九字',9000);",
            )
            .unwrap();
        let rows = PinyinDatabase::open(&path).query_lattice_span_with_mixed_initials(
            &segments(&["c", "ke", "ni", "ni", "ni", "ni", "ni", "j"]),
            32,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "合成八字");
    }
}
