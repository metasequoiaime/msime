//! The wordbooks that ship with the product.
//!
//! Built by `scripts/fetch_wordbooks.py` from ECDICT (MIT), whose `tag` column marks each word
//! against the published syllabuses — zk / gk / cet4 / cet6 / ky / ielts / toefl / gre. The shipped
//! `english.db` carries Chinese glosses and corpus frequency, which is enough to build "the
//! thousand commonest words" but not enough to say a word is on the CET-4 list; that is a
//! published syllabus, and putting the label on a page that nothing behind it supports would be
//! inventing it.
//!
//! The script writes exactly the shape [`Wordbook`] deserialises, so reading a bundled book is
//! `serde_json::from_slice` and this crate needs no CSV parser, no SQLite driver and no new
//! dependency. The eight books together are about 3 MB.
//!
//! A bundled book is read-only: [`super::library::WordbookLibrary`] never sees it, and
//! [`super::session`] refuses to delete one. Its review progress lives in the same store as an
//! imported book's, keyed by the same id.

use super::wordbook::Wordbook;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The directory name the staging scripts publish under, inside the staging root.
///
/// A sibling rather than a member: `ResourceStore::verify` requires the pinned resource directory
/// to hold exactly the dictionary artifacts, so a wordbook inside it would break the check whose
/// job is to prove a shipped dictionary is intact. `settled-model` is a sibling for the same
/// reason.
pub const DIRECTORY: &str = "wordbooks";

/// The largest bundled book that will be read. The biggest today (GRE, 7 460 words) is under 1 MB.
const MAX_BOOK_BYTES: u64 = 8 * 1024 * 1024;

/// The books this build offers, in the order the picker should list them.
///
/// Ordered by difficulty rather than alphabetically: a learner picking a book is choosing a level,
/// and 中考 next to 托福 in an alphabetical list says nothing about which to start with.
pub const ORDER: [&str; 8] = ["zk", "gk", "cet4", "cet6", "ky", "ielts", "toefl", "gre"];

#[derive(Debug, thiserror::Error)]
pub enum BuiltinWordbookError {
    #[error("bundled wordbook storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid bundled wordbook document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bundled wordbook is invalid")]
    InvalidWordbook,
}

/// Where the bundled books live, given the host's staging root.
///
/// The root that holds `EngineResources/`, not that directory itself — see [`DIRECTORY`].
pub fn directory(resources: &Path) -> PathBuf {
    resources.join(DIRECTORY)
}

/// Every bundled book the host has staged, in [`ORDER`].
///
/// A host that ships none — or a build where the fetch script was never run — gets an empty list
/// rather than an error: the page then offers only imported books, which is the state this feature
/// shipped in before the books existed.
///
/// A book that is present but unreadable IS an error. The user can see it in the picker, and a
/// row that silently disappears is worse than one that explains itself.
pub fn load(resources: &Path) -> Result<Vec<Wordbook>, BuiltinWordbookError> {
    let root = directory(resources);
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut books = Vec::new();
    for id in ORDER {
        let path = root.join(format!("{id}.json"));
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.len() > MAX_BOOK_BYTES => {
                return Err(BuiltinWordbookError::InvalidWordbook);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        let file = std::fs::File::open(&path)?;
        let mut bytes = Vec::new();
        file.take(MAX_BOOK_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BOOK_BYTES {
            return Err(BuiltinWordbookError::InvalidWordbook);
        }
        let book: Wordbook = serde_json::from_slice(&bytes)?;
        // The id is the store key for this book's review progress. A document whose stored id
        // disagrees with its filename would record answers under a book nothing can open again.
        if book.id != id || !book.is_valid() {
            return Err(BuiltinWordbookError::InvalidWordbook);
        }
        books.push(book);
    }
    Ok(books)
}

/// Whether `id` names a bundled book, whether or not this host staged it.
///
/// Asked before a delete: the answer must not depend on what happens to be on disk, or a host that
/// failed to stage its books would let the user delete one and then find it back after the next
/// install.
pub fn is_builtin(id: &str) -> bool {
    ORDER.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocabulary::wordbook::WordbookEntry;

    fn book(id: &str, name: &str) -> Wordbook {
        Wordbook {
            id: id.to_owned(),
            name: name.to_owned(),
            entries: vec![WordbookEntry {
                word: "ubiquitous".to_owned(),
                phonetic: "/juːˈbɪkwɪtəs/".to_owned(),
                meaning: "adj. 无处不在的".to_owned(),
            }],
        }
    }

    fn staged(books: &[Wordbook]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join(DIRECTORY);
        std::fs::create_dir_all(&directory).unwrap();
        for book in books {
            std::fs::write(
                directory.join(format!("{}.json", book.id)),
                serde_json::to_vec(book).unwrap(),
            )
            .unwrap();
        }
        root
    }

    #[test]
    fn a_host_with_nothing_staged_offers_no_bundled_books() {
        let root = tempfile::tempdir().unwrap();
        assert!(load(root.path()).unwrap().is_empty());
    }

    #[test]
    fn staged_books_come_back_in_difficulty_order_not_alphabetical() {
        // Written in the opposite order on purpose: the picker's order is the module's, not the
        // filesystem's.
        let root = staged(&[
            book("toefl", "托福"),
            book("cet4", "CET-4"),
            book("zk", "中考"),
        ]);
        assert_eq!(
            load(root.path())
                .unwrap()
                .iter()
                .map(|b| b.id.as_str())
                .collect::<Vec<_>>(),
            vec!["zk", "cet4", "toefl"]
        );
    }

    #[test]
    fn a_partially_staged_host_offers_what_it_has() {
        let root = staged(&[book("cet4", "CET-4")]);
        let books = load(root.path()).unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].name, "CET-4");
    }

    #[test]
    fn a_book_whose_stored_id_disagrees_with_its_filename_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join(DIRECTORY);
        std::fs::create_dir_all(&directory).unwrap();
        // Answers would be recorded under a book nothing can open again.
        std::fs::write(
            directory.join("cet4.json"),
            serde_json::to_vec(&book("cet6", "错位")).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load(root.path()),
            Err(BuiltinWordbookError::InvalidWordbook)
        ));
    }

    #[test]
    fn a_damaged_book_is_reported_rather_than_skipped() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join(DIRECTORY);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("cet4.json"), b"{\"id\":\"cet4\"}").unwrap();
        // The user can see it in the picker; a row that silently disappears explains nothing.
        assert!(load(root.path()).is_err());
    }

    #[test]
    fn the_bundled_names_are_known_without_looking_at_the_disk() {
        assert!(is_builtin("cet4"));
        assert!(is_builtin("gre"));
        assert!(!is_builtin("user-1"));
        assert!(!is_builtin(""));
        // Every shipped id has to be one the store can key a card on.
        assert!(ORDER
            .iter()
            .all(|id| crate::vocabulary::wordbook::id_is_well_formed(id)));
    }
}
