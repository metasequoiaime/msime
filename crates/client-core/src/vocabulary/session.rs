//! One review session's worth of state, and the actions that change it.
//!
//! This is the whole of 背单词 as a host sees it: hand in a day and an action, get back everything
//! the page draws. Assembling it means joining the progress document against the selected
//! wordbook, and that join lives here rather than in each host because six hosts each doing it
//! their own way is six chances for the counts on a page to disagree with the cards it deals.
//!
//! Both consumers are shims over this: the C ABI entry point parses JSON into [`ReviewAction`] and
//! serialises [`ReviewStatus`] back, and the Tauri command layer calls the same function and lets
//! serde render it. Neither owns a rule.

use super::builtin::{self, BuiltinWordbookError};
use super::import as wordbook_import;
use super::library::{WordbookLibrary, WordbookLibraryError, WordbookSummary};
use super::progress::{
    build_queue, VocabularyProgressError, VocabularyProgressStore, VocabularyReviewSettings,
};
use super::schedule::ReviewGrade;
use super::wordbook::{Wordbook, WordbookEntry};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The largest imported word list this layer will parse, in bytes.
///
/// A five-thousand-word CET book is a few hundred kilobytes of text; this is the ceiling on the
/// envelope, and [`wordbook_import`] applies its own row and entry limits underneath.
pub const MAX_IMPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ReviewSessionError {
    #[error("invalid vocabulary review day")]
    InvalidDay,
    #[error("{0}")]
    Progress(#[from] VocabularyProgressError),
    #[error("{0}")]
    Library(#[from] WordbookLibraryError),
    #[error("{0}")]
    Import(#[from] wordbook_import::WordbookImportError),
    #[error("vocabulary wordbook is not in the library")]
    UnknownWordbook,
    #[error("{0}")]
    Builtin(#[from] BuiltinWordbookError),
    #[error("a bundled vocabulary wordbook cannot be deleted")]
    BuiltinNotRemovable,
}

/// What the page draws.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatus {
    pub wordbooks: Vec<WordbookSummary>,
    pub settings: VocabularyReviewSettings,
    /// 今日待复习: cards already waiting, before the new-card allowance is added.
    pub due: usize,
    /// 已完成: answers recorded today.
    pub answered_today: u32,
    pub introducing: usize,
    pub remaining: usize,
    /// The session queue in the order it should be shown.
    pub queue: Vec<WordbookEntry>,
}

/// Something the user did on the page.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewAction {
    Load,
    Answer {
        word: String,
        /// The 认识 button. False is 不认识 and returns the card to the same day.
        known: bool,
    },
    SetSettings {
        wordbook: String,
        new_per_day: u32,
        session_limit: u32,
    },
    Import {
        name: String,
        text: String,
    },
    Remove {
        wordbook: String,
    },
    Reset,
}

/// Apply `action` on `day` and return the resulting status.
///
/// `day` is validated before anything is dispatched, not after. Every action ends by assembling
/// the status for that day, and checking there meant an import wrote a book to disk and only then
/// reported the day was unreadable — a refused request that had already changed the library.
pub fn apply(
    directory: &Path,
    resources: &Path,
    day: &str,
    action: ReviewAction,
) -> Result<ReviewStatus, ReviewSessionError> {
    if !crate::calendar::is_valid_day(day) {
        return Err(ReviewSessionError::InvalidDay);
    }
    let library = WordbookLibrary::new(directory);
    let store = VocabularyProgressStore::new(directory);

    match action {
        ReviewAction::Load => {}
        ReviewAction::Answer { word, known } => {
            let document = store.load()?;
            let book = selected_book(&library, resources, &document.settings.wordbook)?
                .ok_or(ReviewSessionError::UnknownWordbook)?;
            let grade = if known {
                ReviewGrade::Known
            } else {
                ReviewGrade::Unknown
            };
            store.answer(&book, &word, grade, day)?;
        }
        ReviewAction::SetSettings {
            wordbook,
            new_per_day,
            session_limit,
        } => {
            store.set_settings(VocabularyReviewSettings {
                wordbook,
                new_per_day,
                session_limit,
            })?;
        }
        ReviewAction::Import { name, text } => {
            // Validate the existing progress before writing the new book. Import spans two
            // stores; if the progress document is damaged, returning an error after the library
            // write would leave a book behind even though the action failed.
            let document = store.load()?;
            let report = wordbook_import::parse(&text, MAX_IMPORT_BYTES)?;
            // The id is minted here and never taken from the file. A book keys the review
            // progress, so a file that named its own id could inherit or destroy the schedule of a
            // book the user imported earlier.
            let id = format!("user-{}", crate::uuid::Uuid::new_v4().simple());
            let book = library.import(&name, report.entries, &id)?;
            // Selecting it is the only useful next step; leaving the user to pick the book they
            // just imported out of a list is a step with exactly one right answer.
            store.set_settings(VocabularyReviewSettings {
                wordbook: book.id,
                ..document.settings
            })?;
        }
        ReviewAction::Remove { wordbook } => {
            // Validate progress before deleting the book. Removal spans two stores; if the
            // progress document is damaged, returning an error after the library write would
            // still destroy the user's imported material.
            let document = store.load()?;
            // Checked by name rather than by what is on disk: a host that failed to stage its
            // books would otherwise let the user delete one and find it back after the next
            // install, with its review progress already gone.
            if builtin::is_builtin(&wordbook) {
                return Err(ReviewSessionError::BuiltinNotRemovable);
            }
            library.remove(&wordbook)?;
            // The book is gone, so its schedule is unreachable. Leaving it would grow the progress
            // document forever and would silently return if the same id ever came back.
            store.reset_wordbook(&wordbook)?;
            if document.settings.wordbook == wordbook {
                store.set_settings(VocabularyReviewSettings {
                    wordbook: String::new(),
                    ..document.settings
                })?;
            }
        }
        ReviewAction::Reset => {
            // Only the progress. The imported books are the user's own material and are deleted
            // one at a time, deliberately: the button says 清空进度, not 删除词表.
            let settings = store.load()?.settings;
            store.reset()?;
            store.set_settings(settings)?;
        }
    }

    status(directory, resources, day)
}

/// The selected book, bundled or imported. `Ok(None)` when nothing is selected or it has gone.
///
/// Bundled first, because a bundled id is reserved: an imported book can never take one, so there
/// is no order in which the two could disagree about which book a stored card belongs to.
fn selected_book(
    library: &WordbookLibrary,
    resources: &Path,
    id: &str,
) -> Result<Option<Wordbook>, ReviewSessionError> {
    if id.is_empty() {
        return Ok(None);
    }
    if builtin::is_builtin(id) {
        return Ok(builtin::load(resources)?.into_iter().find(|b| b.id == id));
    }
    Ok(library.load(id)?)
}

/// The status for `day`, changing nothing.
pub fn status(
    directory: &Path,
    resources: &Path,
    day: &str,
) -> Result<ReviewStatus, ReviewSessionError> {
    if !crate::calendar::is_valid_day(day) {
        return Err(ReviewSessionError::InvalidDay);
    }
    let library = WordbookLibrary::new(directory);
    let store = VocabularyProgressStore::new(directory);
    // Bundled books first: they are the ones a fresh profile can start from, and the picker should
    // not make a user scroll past their own imports to find 中考.
    let builtins = builtin::load(resources)?;
    let mut wordbooks = Vec::with_capacity(builtins.len());
    wordbooks.extend(builtins.iter().map(|book| WordbookSummary {
        id: book.id.clone(),
        name: book.name.clone(),
        total: book.entries.len(),
        builtin: true,
    }));
    wordbooks.extend(library.list()?);
    let document = store.load()?;
    let settings = document.settings.clone();

    // A selected book that is no longer in the library is reported as an empty queue rather than
    // as an error: the user deleted it, and the page should offer the picker instead of a failure.
    let imported = if settings.wordbook.is_empty() || builtin::is_builtin(&settings.wordbook) {
        None
    } else {
        library.load(&settings.wordbook)?
    };
    let selected = builtins
        .iter()
        .find(|book| book.id == settings.wordbook)
        .or(imported.as_ref());

    let (queue, due, introducing, remaining) = match selected {
        None => (Vec::new(), 0, 0, 0),
        Some(book) => {
            let built = build_queue(
                &document,
                book,
                day,
                settings.new_per_day as usize,
                settings.session_limit as usize,
            )
            .ok_or(ReviewSessionError::InvalidDay)?;
            let mut cards = Vec::with_capacity(built.words.len());
            cards.extend(
                built
                    .words
                    .iter()
                    .filter_map(|word| book.entry(word).cloned()),
            );
            (cards, built.due, built.introducing, built.remaining)
        }
    };

    Ok(ReviewStatus {
        wordbooks,
        settings,
        due,
        answered_today: document.answered_on(day),
        introducing,
        remaining,
        queue,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-09-23";
    const LIST: &str = "alpha,/a/,adj. 甲\nbeta,adj. 乙\ngamma,adj. 丙\n";

    fn directory() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn import(path: &Path, name: &str) -> ReviewStatus {
        apply(
            path,
            path,
            TODAY,
            ReviewAction::Import {
                name: name.to_owned(),
                text: LIST.to_owned(),
            },
        )
        .unwrap()
    }

    #[test]
    fn a_fresh_directory_has_no_books_and_no_queue() {
        let root = directory();
        let status = status(root.path(), root.path(), TODAY).unwrap();
        assert!(status.wordbooks.is_empty());
        assert_eq!(status.settings.wordbook, "");
        assert_eq!(status.due, 0);
        assert!(status.queue.is_empty());
    }

    #[test]
    fn importing_selects_the_book_it_created() {
        let root = directory();
        let status = import(root.path(), "合成词表");
        assert_eq!(status.wordbooks.len(), 1);
        assert_eq!(status.wordbooks[0].name, "合成词表");
        assert_eq!(status.wordbooks[0].total, 3);
        assert!(status.wordbooks[0].id.starts_with("user-"));
        assert_eq!(status.settings.wordbook, status.wordbooks[0].id);
        assert_eq!(status.introducing, 3);
        assert_eq!(status.queue.len(), 3);
        assert_eq!(status.queue[0].word, "alpha");
        assert_eq!(status.queue[0].phonetic, "/a/");
        assert_eq!(
            status.queue[1].phonetic, "",
            "two columns means no phonetic"
        );
    }

    #[test]
    fn two_imports_do_not_share_an_identifier() {
        let root = directory();
        let first = import(root.path(), "甲表").wordbooks[0].id.clone();
        let status = import(root.path(), "乙表");
        assert_eq!(status.wordbooks.len(), 2);
        assert_ne!(
            first, status.settings.wordbook,
            "a second import must not inherit the first book's schedule"
        );
    }

    #[test]
    fn import_does_not_leave_a_book_when_progress_is_corrupt() {
        let root = directory();
        std::fs::write(
            root.path().join("vocabulary-progress.json"),
            b"corrupt synthetic progress",
        )
        .unwrap();

        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                TODAY,
                ReviewAction::Import {
                    name: "合成词表".to_owned(),
                    text: LIST.to_owned(),
                },
            ),
            Err(ReviewSessionError::Progress(_))
        ));
        assert!(WordbookLibrary::new(root.path()).list().unwrap().is_empty());
    }

    #[test]
    fn remove_does_not_delete_a_book_when_progress_is_corrupt() {
        let root = directory();
        let book = import(root.path(), "合成词表").settings.wordbook;
        std::fs::write(
            root.path().join("vocabulary-progress.json"),
            b"corrupt synthetic progress",
        )
        .unwrap();

        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                TODAY,
                ReviewAction::Remove {
                    wordbook: book.clone(),
                },
            ),
            Err(ReviewSessionError::Progress(_))
        ));
        assert!(WordbookLibrary::new(root.path())
            .load(&book)
            .unwrap()
            .is_some());
    }

    #[test]
    fn recall_leaves_todays_queue_and_a_lapse_stays_in_it() {
        let root = directory();
        import(root.path(), "合成词表");

        let answered = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "alpha".to_owned(),
                known: true,
            },
        )
        .unwrap();
        assert_eq!(answered.answered_today, 1);
        assert_eq!(answered.queue[0].word, "beta");

        let failed = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "beta".to_owned(),
                known: false,
            },
        )
        .unwrap();
        assert_eq!(failed.answered_today, 2);
        assert!(
            failed.queue.iter().any(|card| card.word == "beta"),
            "a failed card stays in the session it was failed in"
        );
    }

    #[test]
    fn the_next_day_starts_its_own_count_and_the_recalled_card_is_due() {
        let root = directory();
        import(root.path(), "合成词表");
        apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "alpha".to_owned(),
                known: true,
            },
        )
        .unwrap();

        let tomorrow = status(root.path(), root.path(), "2026-09-24").unwrap();
        assert_eq!(tomorrow.answered_today, 0);
        assert_eq!(tomorrow.due, 1);
        assert_eq!(tomorrow.queue[0].word, "alpha");
    }

    #[test]
    fn resetting_keeps_the_books_and_the_selection() {
        let root = directory();
        let book = import(root.path(), "合成词表").settings.wordbook;
        apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "alpha".to_owned(),
                known: true,
            },
        )
        .unwrap();

        let reset = apply(root.path(), root.path(), TODAY, ReviewAction::Reset).unwrap();
        assert_eq!(reset.answered_today, 0);
        assert_eq!(reset.settings.wordbook, book, "清空进度 is not 删除词表");
        assert_eq!(reset.wordbooks.len(), 1);
        assert_eq!(reset.queue[0].word, "alpha", "the card is new again");
    }

    #[test]
    fn removing_a_book_takes_its_schedule_and_clears_the_selection() {
        let root = directory();
        let book = import(root.path(), "合成词表").settings.wordbook;
        apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "alpha".to_owned(),
                known: true,
            },
        )
        .unwrap();

        let removed = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Remove {
                wordbook: book.clone(),
            },
        )
        .unwrap();
        assert!(removed.wordbooks.is_empty());
        assert_eq!(removed.settings.wordbook, "");
        assert!(removed.queue.is_empty());

        let document = VocabularyProgressStore::new(root.path()).load().unwrap();
        assert!(
            document.card(&book, "alpha").is_none(),
            "an unreachable schedule would grow the document forever"
        );
    }

    #[test]
    fn a_selected_book_that_is_gone_offers_the_picker_rather_than_an_error() {
        let root = directory();
        let book = import(root.path(), "合成词表").settings.wordbook;
        WordbookLibrary::new(root.path()).remove(&book).unwrap();

        let status = status(root.path(), root.path(), TODAY).unwrap();
        assert!(status.queue.is_empty());
        assert_eq!(status.due, 0);
    }

    #[test]
    fn an_unparseable_day_is_refused_before_the_library_is_touched() {
        let root = directory();
        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                "2026-13-01",
                ReviewAction::Import {
                    name: "坏日期".to_owned(),
                    text: LIST.to_owned(),
                },
            ),
            Err(ReviewSessionError::InvalidDay)
        ));
        assert!(
            status(root.path(), root.path(), TODAY)
                .unwrap()
                .wordbooks
                .is_empty(),
            "a refused request must not have written a book"
        );
    }

    #[test]
    fn a_file_with_no_usable_rows_is_refused_rather_than_stored_empty() {
        let root = directory();
        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                TODAY,
                ReviewAction::Import {
                    name: "空的".to_owned(),
                    text: "# 只有注释\n".to_owned(),
                },
            ),
            Err(ReviewSessionError::Import(_))
        ));
        assert!(status(root.path(), root.path(), TODAY)
            .unwrap()
            .wordbooks
            .is_empty());
    }

    #[test]
    fn answering_without_a_selected_book_is_refused() {
        let root = directory();
        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                TODAY,
                ReviewAction::Answer {
                    word: "alpha".to_owned(),
                    known: true,
                },
            ),
            Err(ReviewSessionError::Library(_) | ReviewSessionError::UnknownWordbook)
        ));
    }

    fn stage_builtin(root: &Path, id: &str, name: &str, words: &[&str]) {
        let directory = root.join(super::builtin::DIRECTORY);
        std::fs::create_dir_all(&directory).unwrap();
        let book = Wordbook {
            id: id.to_owned(),
            name: name.to_owned(),
            entries: words
                .iter()
                .map(|word| WordbookEntry {
                    word: (*word).to_owned(),
                    phonetic: String::new(),
                    meaning: "adj. 合成释义".to_owned(),
                })
                .collect(),
        };
        std::fs::write(
            directory.join(format!("{id}.json")),
            serde_json::to_vec(&book).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_fresh_profile_can_start_from_a_bundled_book() {
        let root = directory();
        stage_builtin(root.path(), "cet4", "CET-4", &["alpha", "beta", "gamma"]);

        // 没有任何导入，页面也不该是空的。
        let listed = status(root.path(), root.path(), TODAY).unwrap();
        assert_eq!(listed.wordbooks.len(), 1);
        assert_eq!(listed.wordbooks[0].id, "cet4");
        assert!(
            listed.wordbooks[0].builtin,
            "a bundled book is not deletable"
        );

        let chosen = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::SetSettings {
                wordbook: "cet4".to_owned(),
                new_per_day: 20,
                session_limit: 200,
            },
        )
        .unwrap();
        assert_eq!(chosen.introducing, 3);
        assert_eq!(chosen.queue[0].word, "alpha");

        let answered = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::Answer {
                word: "alpha".to_owned(),
                known: true,
            },
        )
        .unwrap();
        assert_eq!(answered.answered_today, 1);
        assert_eq!(answered.queue[0].word, "beta");
    }

    #[test]
    fn bundled_books_are_listed_before_imports_and_cannot_be_deleted() {
        let root = directory();
        stage_builtin(root.path(), "cet4", "CET-4", &["alpha"]);
        import(root.path(), "我的词表");

        let listed = status(root.path(), root.path(), TODAY).unwrap();
        assert_eq!(
            listed
                .wordbooks
                .iter()
                .map(|b| b.builtin)
                .collect::<Vec<_>>(),
            vec![true, false],
            "a fresh profile should not scroll past its own imports to find 中考"
        );

        assert!(matches!(
            apply(
                root.path(),
                root.path(),
                TODAY,
                ReviewAction::Remove {
                    wordbook: "cet4".to_owned()
                },
            ),
            Err(ReviewSessionError::BuiltinNotRemovable)
        ));
        // 删不掉，也就不会把进度一起带走。
        assert_eq!(
            status(root.path(), root.path(), TODAY)
                .unwrap()
                .wordbooks
                .len(),
            2
        );
    }

    #[test]
    fn settings_round_trip_through_one_action() {
        let root = directory();
        let book = import(root.path(), "合成词表").settings.wordbook;
        let status = apply(
            root.path(),
            root.path(),
            TODAY,
            ReviewAction::SetSettings {
                wordbook: book.clone(),
                new_per_day: 1,
                session_limit: 50,
            },
        )
        .unwrap();
        assert_eq!(status.settings.new_per_day, 1);
        assert_eq!(status.settings.session_limit, 50);
        assert_eq!(
            status.introducing, 1,
            "the new allowance takes effect immediately"
        );
    }
}
