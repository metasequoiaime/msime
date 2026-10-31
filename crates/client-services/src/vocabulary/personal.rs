//! 候选窗里的「加入生词本」。
//!
//! 六个宿主的候选窗都只做一件事：把右键（或长按）的那个候选的原文、显示着的释义和读音交进来，再把这里回答的那句提示原样显示出来。释义怎么变成卡片、同一个词怎么合并、满了怎么办、提示怎么说，都在共享层定，宿主之间不会各说各的。

use super::library::{
    personal_entry_from_gloss, PersonalAddOutcome, WordbookLibrary, WordbookLibraryError,
};
use serde::Serialize;
use std::path::Path;

/// 一次「加入生词本」的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalGlossOutcome {
    Added,
    Merged,
    AlreadyPresent,
    /// 生词本已经满了，这个词没有加进去。
    Full,
    /// 这条释义做不成卡片：空的，或者比词头的长度上限还长（例如整句的 AI 译文）。
    Invalid,
}

/// 宿主拿到的回答：结果和要显示给用户的一句提示。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PersonalGlossReply {
    pub outcome: PersonalGlossOutcome,
    /// 宿主在候选窗旁的提示里原样显示这句话。
    pub notice: &'static str,
}

impl PersonalGlossReply {
    fn new(outcome: PersonalGlossOutcome) -> Self {
        let notice = match outcome {
            PersonalGlossOutcome::Added | PersonalGlossOutcome::Merged => "已加入生词本",
            PersonalGlossOutcome::AlreadyPresent => "已在生词本里",
            PersonalGlossOutcome::Full => "生词本已满",
            PersonalGlossOutcome::Invalid => "这条释义不能加入生词本",
        };
        Self { outcome, notice }
    }
}

/// 把候选 `candidate` 的释义 `gloss` 加进 `directory` 下的生词本。`pronunciation` 是宿主随释义显示的读音，几行时只用第一行，可以为空。
///
/// 满了和释义不合格是用户能看懂的结果，不是错误；只有存储本身出问题时才返回 `Err`。会取词书目录的文件锁、读写生词本，宿主在用户点了菜单之后调用一次，不要在每次按键时调用。
pub fn add_gloss(
    directory: &Path,
    candidate: &str,
    gloss: &str,
    pronunciation: &str,
) -> Result<PersonalGlossReply, WordbookLibraryError> {
    let reading = pronunciation
        .split(['\n', '\u{2028}'])
        .next()
        .unwrap_or_default();
    let Some(entry) = personal_entry_from_gloss(candidate, gloss, reading) else {
        return Ok(PersonalGlossReply::new(PersonalGlossOutcome::Invalid));
    };
    let outcome = match WordbookLibrary::new(directory).add_personal(entry) {
        Ok(PersonalAddOutcome::Added) => PersonalGlossOutcome::Added,
        Ok(PersonalAddOutcome::Merged) => PersonalGlossOutcome::Merged,
        Ok(PersonalAddOutcome::AlreadyPresent) => PersonalGlossOutcome::AlreadyPresent,
        Err(WordbookLibraryError::PersonalFull) => PersonalGlossOutcome::Full,
        Err(WordbookLibraryError::InvalidEntry) => PersonalGlossOutcome::Invalid,
        Err(error) => return Err(error),
    };
    Ok(PersonalGlossReply::new(outcome))
}

/// 生词本导出成的文本，给设置页的「导出生词本」。还没有生词本时是 `None`。
pub fn export(directory: &Path) -> Result<Option<String>, WordbookLibraryError> {
    WordbookLibrary::new(directory).export_personal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocabulary::library::PERSONAL_ID;

    #[test]
    fn a_gloss_is_added_once_and_then_reported_as_present() {
        let root = tempfile::tempdir().unwrap();
        let first = add_gloss(root.path(), "面试", "interview; audition", "/ˈɪntəvjuː/\n").unwrap();
        assert_eq!(first.outcome, PersonalGlossOutcome::Added);
        assert_eq!(first.notice, "已加入生词本");
        let again = add_gloss(root.path(), "面试", "interview", "").unwrap();
        assert_eq!(again.outcome, PersonalGlossOutcome::AlreadyPresent);
        let merged = add_gloss(root.path(), "面谈", "interview", "").unwrap();
        assert_eq!(merged.outcome, PersonalGlossOutcome::Merged);

        let book = WordbookLibrary::new(root.path())
            .load(PERSONAL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(book.entries[0].phonetic, "/ˈɪntəvjuː/");
        assert_eq!(book.entries[0].meaning, "面试；面谈");
        assert!(export(root.path()).unwrap().unwrap().contains("interview"));
    }

    #[test]
    fn an_unusable_gloss_is_an_answer_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let reply = add_gloss(root.path(), "一句话", &"long ".repeat(30), "").unwrap();
        assert_eq!(reply.outcome, PersonalGlossOutcome::Invalid);
        assert_eq!(reply.notice, "这条释义不能加入生词本");
        assert_eq!(export(root.path()).unwrap(), None);
    }
}
