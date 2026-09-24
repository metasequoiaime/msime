//! English glosses for the Chinese candidates english.db does not answer.
//!
//! english.db's Chinese-to-English side is ECDICT reversed and deliberately conservative: 2 to 6 characters, and only
//! Chinese words that translate a common English word, about 18 600 of them. The Japanese offline gloss, built from the
//! Chinese side, covers far more, so many candidates showed a Japanese line and no English one, and no single character
//! had English at all. Two tables beside the resource directory fill those gaps, and never replace an english.db or
//! learned answer:
//!
//! - `character-glosses/zh-en.db`, Unihan's kDefinition (scripts/build_character_glosses.py): meaning-first definitions
//!   of single characters, asked first for a one-character candidate.
//! - `word-glosses/zh-en.db`, CC-CEDICT (scripts/build_word_glosses.py): 98 000 words and characters written from the
//!   Chinese side, asked for anything still empty. Its entries for one character are ordered by pinyin rather than by use
//!   (要 yāo before yào), which is why characters ask Unihan first.
//!
//! Both have the offline-gloss shape, so the bridge's candidate_target_glosses reads them.

use std::path::{Path, PathBuf};

const CHARACTER_TABLE: &str = "character-glosses";
const WORD_TABLE: &str = "word-glosses";

/// `<directory>/zh-en.db` beside the resource directory, when it is installed.
pub(crate) fn database_beside(resources: &Path, directory: &str) -> Option<PathBuf> {
    let path = resources.parent()?.join(directory).join("zh-en.db");
    path.is_file().then_some(path)
}

/// Fill the empty glosses in place; `glosses` is parallel to `candidates`.
///
/// Anything wrong with a table — missing, unreadable, another version — leaves the glosses as they were: the words
/// english.db answered must not be lost to a problem with a supplement.
pub(crate) fn fill(resources: &Path, candidates: &[(String, u8)], glosses: &mut [String]) {
    fill_from(resources, CHARACTER_TABLE, candidates, glosses, |text| {
        text.chars().count() == 1
    });
    fill_from(resources, WORD_TABLE, candidates, glosses, |_| true);
}

fn fill_from(
    resources: &Path,
    directory: &str,
    candidates: &[(String, u8)],
    glosses: &mut [String],
    wanted: impl Fn(&str) -> bool,
) {
    let wanted = candidates
        .iter()
        .zip(glosses.iter())
        .enumerate()
        .filter(|(_, ((text, _), gloss))| gloss.is_empty() && wanted(text))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if wanted.is_empty() {
        return;
    }
    let Some(database) = database_beside(resources, directory) else {
        return;
    };
    let Some(database) = database.to_str() else {
        return;
    };
    let asked = wanted
        .iter()
        .map(|index| candidates[*index].clone())
        .collect::<Vec<_>>();
    let Ok(found) = msime_engine::host::candidate_target_glosses(database, "en", &asked) else {
        return;
    };
    for (index, gloss) in wanted.into_iter().zip(found) {
        if !gloss.is_empty() {
            glosses[index] = gloss;
        }
    }
}
