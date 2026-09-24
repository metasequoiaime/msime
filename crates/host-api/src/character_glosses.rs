//! English glosses for single Chinese characters, which english.db leaves out on purpose.
//!
//! english.db's Chinese-to-English side is ECDICT reversed, and it keeps only 2 to 6 characters because a single
//! character appears in too many English words' translations for the reverse to mean anything. So 爱, 天 and 我 had no
//! English line at all. `character-glosses/zh-en.db` beside the resource directory, built from Unihan's kDefinition by
//! scripts/build_character_glosses.py, has the offline-gloss shape and fills exactly those: a candidate of one
//! character that english.db (and the user's learned glosses) did not answer.

use std::path::{Path, PathBuf};

/// `character-glosses/zh-en.db` beside the resource directory, when it is installed.
pub(crate) fn database_beside(resources: &Path) -> Option<PathBuf> {
    let path = resources
        .parent()?
        .join("character-glosses")
        .join("zh-en.db");
    path.is_file().then_some(path)
}

/// Fill the empty glosses of single-character candidates in place; `glosses` is parallel to `candidates`.
///
/// Anything wrong with the table — missing, unreadable, another version — leaves the glosses as they were: the
/// words english.db answered must not be lost to a problem with a supplement.
pub(crate) fn fill_single_characters(
    resources: &Path,
    candidates: &[(String, u8)],
    glosses: &mut [String],
) {
    let wanted = candidates
        .iter()
        .zip(glosses.iter())
        .enumerate()
        .filter(|(_, ((text, _), gloss))| gloss.is_empty() && text.chars().count() == 1)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if wanted.is_empty() {
        return;
    }
    let Some(database) = database_beside(resources) else {
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
