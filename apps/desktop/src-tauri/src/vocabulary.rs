//! 背单词的命令。
//!
//! 这里的每一个命令都只是一层薄壳：算出用户的本地日期、指定一个动作，再把两者交给 `msime_client_services::vocabulary::session`。规则（一个动作做什么、队列怎样构建、一次遗忘的代价是什么）都放在那里，因为原生宿主调用的 C ABI 入口走的是同一个函数，两者不能各走各的。
//!
//! 每个命令都返回完整的状态，而不是什么都不返回，这样页面就能始终只保持一个在途请求，不必在每次改动之后再自己读一次。

use crate::{CommandError, VocabularyState};
use msime_client_services::vocabulary::session::{self, ReviewAction, ReviewStatus};

impl From<session::ReviewSessionError> for CommandError {
    fn from(value: session::ReviewSessionError) -> Self {
        use msime_client_services::vocabulary::library::WordbookLibraryError;
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
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    action: ReviewAction,
) -> Result<ReviewStatus, CommandError> {
    let directory = state.0.clone();
    let resources = crate::vocabulary_resources(&app, &state.1);
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
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
) -> Result<ReviewStatus, CommandError> {
    run(app, state, ReviewAction::Load).await
}

#[tauri::command]
pub async fn answer_vocabulary_card(
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    wordbook: String,
    word: String,
    known: bool,
) -> Result<ReviewStatus, CommandError> {
    run(
        app,
        state,
        ReviewAction::Answer {
            wordbook,
            word,
            known,
        },
    )
    .await
}

#[tauri::command]
pub async fn set_vocabulary_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    wordbook: String,
    new_per_day: u32,
    session_limit: u32,
) -> Result<ReviewStatus, CommandError> {
    run(
        app,
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
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    name: String,
    text: String,
) -> Result<ReviewStatus, CommandError> {
    run(app, state, ReviewAction::Import { name, text }).await
}

#[tauri::command]
pub async fn remove_vocabulary_wordbook(
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    wordbook: String,
) -> Result<ReviewStatus, CommandError> {
    run(app, state, ReviewAction::Remove { wordbook }).await
}

#[tauri::command]
pub async fn remove_personal_vocabulary_word(
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
    word: String,
) -> Result<ReviewStatus, CommandError> {
    run(app, state, ReviewAction::RemovePersonalWord { word }).await
}

/// 「导出生词本」：生词本的 CSV 文本，还没有生词本时是 `None`。页面交给宿主的保存对话框或下载链接。
#[tauri::command]
pub async fn export_personal_vocabulary(
    state: tauri::State<'_, VocabularyState>,
) -> Result<Option<String>, CommandError> {
    let directory = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        msime_client_services::vocabulary::personal::export(&directory)
            .map_err(|error| session::ReviewSessionError::from(error).into())
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
pub async fn reset_vocabulary_review(
    app: tauri::AppHandle,
    state: tauri::State<'_, VocabularyState>,
) -> Result<ReviewStatus, CommandError> {
    run(app, state, ReviewAction::Reset).await
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
        use msime_client_services::vocabulary::library::WordbookLibraryError;
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
                msime_client_services::vocabulary::import::WordbookImportError::NoUsableRows
            )),
            "wordbook_unreadable"
        );
    }
}
