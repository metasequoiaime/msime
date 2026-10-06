//! 背单词 commands.
//!
//! Every one of these is a shim: it resolves the user's local day, names an action, and hands both
//! to `msime_client_core::vocabulary::session`. The rules — what an action does, how the queue is
//! built, what a lapse costs — live there, because the C ABI entry point the native hosts call
//! goes through the same function and the two must not drift.
//!
//! Each command answers with the whole status rather than nothing, which is what lets the page
//! keep one request in flight instead of following every change with a read of its own.

use crate::{CommandError, VocabularyState};
use msime_client_core::vocabulary::session::{self, ReviewAction, ReviewStatus};

impl From<session::ReviewSessionError> for CommandError {
    fn from(value: session::ReviewSessionError) -> Self {
        use msime_client_core::vocabulary::library::WordbookLibraryError;
        use session::ReviewSessionError;
        Self {
            code: match value {
                ReviewSessionError::InvalidDay => "invalid",
                ReviewSessionError::Import(_) => "wordbook_unreadable",
                ReviewSessionError::Library(WordbookLibraryError::LibraryFull) => "wordbook_full",
                ReviewSessionError::Library(WordbookLibraryError::InvalidWordbook) => "invalid",
                ReviewSessionError::Library(WordbookLibraryError::UnknownWordbook)
                | ReviewSessionError::UnknownWordbook => "wordbook_missing",
                ReviewSessionError::BuiltinNotRemovable => "wordbook_builtin",
                ReviewSessionError::PackNotRemovable => "wordbook_pack",
                _ => "storage",
            },
        }
    }
}

/// The user's local day, as `YYYY-MM-DD`.
///
/// Resolved in this process because it is the one that knows the machine's timezone; the shared
/// layer never derives a day, for the same reason a recorded commit carries one. A machine with no
/// resolvable offset falls back to UTC rather than refusing to review.
fn today() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let date = now.date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

async fn run(
    state: tauri::State<'_, VocabularyState>,
    action: ReviewAction,
) -> Result<ReviewStatus, CommandError> {
    let directory = state.0.clone();
    let resources = state.1.clone();
    let plugins = state.2.clone();
    // Takes a file lock and may read a multi-megabyte book, so it never runs on the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        Ok(session::apply(
            &directory,
            &resources,
            plugins.as_deref(),
            &today(),
            action,
        )?)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
pub async fn load_vocabulary_review(
    state: tauri::State<'_, VocabularyState>,
) -> Result<ReviewStatus, CommandError> {
    run(state, ReviewAction::Load).await
}

#[tauri::command]
pub async fn answer_vocabulary_card(
    state: tauri::State<'_, VocabularyState>,
    word: String,
    known: bool,
) -> Result<ReviewStatus, CommandError> {
    run(state, ReviewAction::Answer { word, known }).await
}

#[tauri::command]
pub async fn set_vocabulary_settings(
    state: tauri::State<'_, VocabularyState>,
    wordbook: String,
    new_per_day: u32,
    session_limit: u32,
) -> Result<ReviewStatus, CommandError> {
    run(
        state,
        ReviewAction::SetSettings {
            wordbook,
            new_per_day,
            session_limit,
        },
    )
    .await
}

#[tauri::command]
pub async fn import_vocabulary_wordbook(
    state: tauri::State<'_, VocabularyState>,
    name: String,
    text: String,
) -> Result<ReviewStatus, CommandError> {
    run(state, ReviewAction::Import { name, text }).await
}

#[tauri::command]
pub async fn remove_vocabulary_wordbook(
    state: tauri::State<'_, VocabularyState>,
    wordbook: String,
) -> Result<ReviewStatus, CommandError> {
    run(state, ReviewAction::Remove { wordbook }).await
}

#[tauri::command]
pub async fn reset_vocabulary_review(
    state: tauri::State<'_, VocabularyState>,
) -> Result<ReviewStatus, CommandError> {
    run(state, ReviewAction::Reset).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resolved_day_is_the_shape_the_shared_layer_accepts() {
        let day = today();
        assert!(
            msime_client_core::calendar::is_valid_day(&day),
            "the shared layer would refuse {day}"
        );
    }

    #[test]
    fn session_failures_carry_a_code_the_page_can_act_on() {
        use msime_client_core::vocabulary::library::WordbookLibraryError;
        use session::ReviewSessionError;

        let code = |error: ReviewSessionError| CommandError::from(error).code;
        assert_eq!(code(ReviewSessionError::InvalidDay), "invalid");
        assert_eq!(code(ReviewSessionError::PackNotRemovable), "wordbook_pack");
        assert_eq!(
            code(ReviewSessionError::UnknownWordbook),
            "wordbook_missing"
        );
        assert_eq!(
            code(ReviewSessionError::Library(
                WordbookLibraryError::LibraryFull
            )),
            "wordbook_full"
        );
        // A file the user picked that turned out not to be a word list is its own answer: "storage"
        // would read as the application being broken rather than the file being wrong.
        assert_eq!(
            code(ReviewSessionError::Import(
                msime_client_core::vocabulary::import::WordbookImportError::NoUsableRows
            )),
            "wordbook_unreadable"
        );
    }
}
