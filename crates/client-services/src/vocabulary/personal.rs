//! 候选窗里的「加入生词本」。
//!
//! 六个宿主的候选窗都只做一件事：把右键（或长按）的那个候选的原文、显示着的释义和读音交进来，再把这里回答的那句提示原样显示出来。释义怎么变成卡片、同一个词怎么合并、满了怎么办、提示怎么说，都在共享层定，宿主之间不会各说各的。

use super::library::{
    personal_entry_from_gloss, PersonalAddOutcome, PersonalImportReport, WordbookLibrary,
    WordbookLibraryError, PERSONAL_ID,
};
use super::wordbook::Wordbook;
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

/// 合并时读的生词本文档的字节上限：生词本最多 5000 词，按释义上限算也不到 2 MB。
const MAX_PUBLISHED_BYTES: usize = 4 * 1024 * 1024;

/// 键盘进程自己记下的生词本，并进这个目录的生词本。
///
/// 鸿蒙从 API 12 起输入法扩展用独立沙箱，键盘写不到设置应用的目录：键盘把「加入生词本」写进自己沙箱里的一本，经共享沙箱镜像发布出来，设置应用导入时调用这里。`published` 是键盘这次发布的那本（`Wordbook` 的 JSON），`previous` 是上次已经并过的那一版，没有时为 `None`。只并进上次以后新出现或改过的词条：用户在设置这边移出或清空过的词，不会因为键盘那本里还留着而再回来。`previous` 读不懂时当作没有。
pub fn merge_published(
    directory: &Path,
    published: &str,
    previous: Option<&str>,
) -> Result<PersonalImportReport, WordbookLibraryError> {
    let parse = |text: &str| -> Option<Wordbook> {
        if text.len() > MAX_PUBLISHED_BYTES {
            return None;
        }
        serde_json::from_str::<Wordbook>(text)
            .ok()
            .filter(|book| book.id == PERSONAL_ID && book.is_valid())
    };
    let book = parse(published).ok_or(WordbookLibraryError::InvalidWordbook)?;
    let before = previous
        .and_then(parse)
        .map(|book| book.entries)
        .unwrap_or_default();
    let fresh = book
        .entries
        .into_iter()
        .filter(|entry| !before.contains(entry))
        .collect();
    WordbookLibrary::new(directory).merge_personal(fresh, PersonalImportReport::default())
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

    #[test]
    fn a_published_keyboard_book_merges_only_what_is_new_since_last_time() {
        let keyboard = tempfile::tempdir().unwrap();
        let settings = tempfile::tempdir().unwrap();
        let keyboard_book = |root: &Path| {
            std::fs::read_to_string(root.join("vocabulary-wordbooks/personal.json")).unwrap()
        };
        add_gloss(keyboard.path(), "面试", "interview", "").unwrap();
        add_gloss(keyboard.path(), "苹果", "apple", "").unwrap();
        let first = keyboard_book(keyboard.path());
        let report = merge_published(settings.path(), &first, None).unwrap();
        assert_eq!(report.added, 2);

        // 设置这边移出一个词以后，键盘那本里还留着它；键盘再加词时，移出的词不能回来。
        WordbookLibrary::new(settings.path())
            .remove_personal_entry("apple")
            .unwrap();
        add_gloss(keyboard.path(), "香蕉", "banana", "").unwrap();
        let second = keyboard_book(keyboard.path());
        let report = merge_published(settings.path(), &second, Some(&first)).unwrap();
        assert_eq!((report.added, report.merged), (1, 0));
        let words: Vec<String> = WordbookLibrary::new(settings.path())
            .load(PERSONAL_ID)
            .unwrap()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.word)
            .collect();
        assert_eq!(words, vec!["interview", "banana"]);

        // 上一版读不懂时当作没有，照常全部合并，不报错。
        assert!(merge_published(settings.path(), &second, Some("not json")).is_ok());
        assert!(matches!(
            merge_published(settings.path(), "{}", None),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }
}
