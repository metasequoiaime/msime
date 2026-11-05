//! 首选切分之外的完整词条；不展开备选切分的短词或组句。

use super::*;

#[derive(Default)]
pub(super) struct Alternatives {
    input: String,
    paths: Vec<Segmentation>,
    inventory: Option<Arc<Inventory>>,
    key: String,
    entries: Vec<LanguageEntry>,
}

impl Alternatives {
    pub(super) fn append(
        &mut self,
        input: &str,
        inventory: &Arc<Inventory>,
        dictionary: CantoneseLexicon<'_>,
        candidates: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
        if self.input != input
            || self
                .inventory
                .as_ref()
                .is_none_or(|cached| !Arc::ptr_eq(cached, inventory))
        {
            self.inventory = Some(inventory.clone());
            self.input.clear();
            self.input.push_str(input);
            // 保留分段器原有的 16 条路径上限；重复查询只复用路径，词条仍读当前用户词库。
            self.paths = syllable::segment(input, inventory);
        }
        let end = input.trim_end_matches('\'').len();
        let mut insertion = candidates
            .iter()
            .position(|row| row.end < end)
            .unwrap_or(candidates.len());
        for reading in self.paths.iter().skip(1) {
            if reading.ends_in_prefix() || reading.end() != end {
                continue;
            }
            reading.key_into(input, reading.syllables.len(), &mut self.key);
            dictionary.lookup_into(&self.key, SPAN_LIMIT, &mut self.entries)?;
            for entry in &self.entries {
                if candidates
                    .iter()
                    .any(|row| row.end == end && row.text == entry.text)
                {
                    continue;
                }
                candidates.insert(
                    insertion,
                    CantoneseCandidate {
                        text: entry.text.clone(),
                        key: self.key.clone(),
                        weight: entry.weight,
                        syllables: reading.syllables.len(),
                        end,
                        composed: false,
                    },
                );
                insertion += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_dictionary::{open_read_only, SCHEMA};
    use rusqlite::Connection;

    #[test]
    fn alternative_full_word_is_selectable_and_boundaries_are_respected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute(
            "INSERT INTO metadata VALUES ('format_version', ?1)",
            [crate::language_dictionary::FORMAT_VERSION.to_string()],
        )
        .unwrap();
        for (key, text, weight) in [
            ("zaa", "甲", 500),
            ("ding", "乙", 500),
            ("aai", "丙", 500),
            ("zaa din gaai", "甲丁戊", 1),
            ("zaa ding", "甲乙", 100),
            ("ngoi", "己", 500),
            ("ngo i", "庚辛", 1),
        ] {
            db.execute(
                "INSERT INTO entries VALUES (?1, ?2, ?3)",
                (key, text, weight),
            )
            .unwrap();
        }
        drop(db);
        let dictionary = open_read_only(&path).unwrap();
        let inventory = Arc::new(Inventory::new([
            "zaa", "ding", "aai", "din", "gaai", "ngoi", "ngo", "i",
        ]));
        for (input, expected, absent) in [
            ("zaadingaai", Some("甲丁戊"), None),
            ("zaading'aai", None, Some("甲丁戊")),
            ("zaadingaaix", None, Some("甲丁戊")),
            ("ngoi", Some("庚辛"), None),
        ] {
            let mut scheme = CantoneseScheme::new(inventory.clone());
            scheme.input = input.to_owned();
            let rows = scheme.candidates(&dictionary).unwrap();
            if let Some(text) = expected {
                let index = rows.iter().position(|row| row.text == text).unwrap();
                assert!(!rows[index].composed);
                assert_eq!(rows[index].end, input.len());
                assert!(rows[..index].iter().all(|row| row.end == input.len()));
                assert!(!scheme.select(&rows[index]));
                assert!(scheme.input().is_empty());
            }
            if let Some(text) = absent {
                assert!(rows.iter().all(|row| row.text != text));
            }
        }
    }
}
