//! English glosses for Chinese candidates from dictionaries written from the Chinese side.
//!
//! english.db's Chinese-to-English side is ECDICT reversed and deliberately conservative: 2 to 6 characters, and only
//! Chinese words that translate a common English word, about 18 600 of them, some of them loose (漂亮 "chic", 电源
//! "ps"). This input method is also a learning tool, so where a dictionary written from the Chinese side has an entry,
//! that entry is shown instead:
//!
//! - `character-glosses/zh-en.db`, Unihan's kDefinition (scripts/build_character_glosses.py): meaning-first definitions
//!   of single characters, asked first for a one-character candidate.
//! - `word-glosses/zh-en.db`, CC-CEDICT (scripts/build_word_glosses.py): 98 000 words and characters, asked for
//!   everything else. Its entries for one character are ordered by pinyin rather than by use (要 yāo before yào), which
//!   is why characters ask Unihan first.
//!
//! A gloss from the user's own glossary is never replaced, and english.db remains the answer for whatever neither table
//! has. Both tables have the offline-gloss shape, so the bridge's candidate_target_glosses reads them; a missing or
//! damaged table leaves the glosses as they were.

use std::path::{Path, PathBuf};

const CHARACTER_TABLE: &str = "character-glosses";
const WORD_TABLE: &str = "word-glosses";

/// `<directory>/zh-en.db` beside the resource directory, when it is installed.
pub(crate) fn database_beside(resources: &Path, directory: &str) -> Option<PathBuf> {
    let path = resources.parent()?.join(directory).join("zh-en.db");
    path.is_file().then_some(path)
}

/// Replace glosses with the tables' entries in place; `glosses` and `learned` are parallel to `candidates`, and a
/// learned gloss is kept. The tables only hold Chinese keys, so English candidates keep their Chinese gloss.
pub(crate) fn prefer(
    resources: &Path,
    candidates: &[(String, u8)],
    glosses: &mut [String],
    learned: &[bool],
) {
    let mut settled = learned.to_vec();
    settled.resize(candidates.len(), true);
    for (directory, wanted) in [
        (
            CHARACTER_TABLE,
            (|text: &str| text.chars().count() == 1) as fn(&str) -> bool,
        ),
        (WORD_TABLE, |_: &str| true),
    ] {
        let asked = (0..candidates.len())
            .filter(|index| !settled[*index] && wanted(&candidates[*index].0))
            .collect::<Vec<_>>();
        for (index, gloss) in lookup(resources, directory, candidates, &asked) {
            glosses[index] = gloss;
            settled[index] = true;
        }
    }
}

/// The non-empty entries of `directory`'s table for the candidates at `indices`.
fn lookup(
    resources: &Path,
    directory: &str,
    candidates: &[(String, u8)],
    indices: &[usize],
) -> Vec<(usize, String)> {
    if indices.is_empty() {
        return Vec::new();
    }
    let Some(database) = database_beside(resources, directory) else {
        return Vec::new();
    };
    let Some(database) = database.to_str() else {
        return Vec::new();
    };
    let asked = indices
        .iter()
        .map(|index| candidates[*index].clone())
        .collect::<Vec<_>>();
    let Ok(found) = msime_engine::host::candidate_target_glosses(database, "en", &asked) else {
        return Vec::new();
    };
    indices
        .iter()
        .copied()
        .zip(found)
        .filter(|(_, gloss)| !gloss.is_empty())
        .collect()
}
