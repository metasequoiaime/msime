//! 候选窗里的「加入生词本」。
//!
//! 六个宿主的候选窗都只做一件事：把右键（或长按）的那个候选的原文、显示着的释义和读音交进来，再把这里回答的那句提示原样显示出来。释义怎么变成卡片、同一个词怎么合并、满了怎么办、提示怎么说，都在共享层定，宿主之间不会各说各的。

use super::library::{
    personal_entry_from_gloss, PersonalAddOutcome, PersonalImportReport, WordbookLibrary,
    WordbookLibraryError, MAX_BOOK_BYTES, PERSONAL_ID,
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

/// 读键盘发布的那本生词本（或设置应用存下的上一版）。文件不在、读不出来、超过单本书的大小上限，或者不是一本有效的生词本时都是 `None`。不跟随符号链接。
fn read_published(path: &Path) -> Option<Wordbook> {
    let file = crate::file_lock::open_private_file(path).ok()?;
    let bytes =
        crate::bounded_io::read_bounded_file_with(file, MAX_BOOK_BYTES, || (), |_| ()).ok()?;
    serde_json::from_slice::<Wordbook>(&bytes)
        .ok()
        .filter(|book| book.id == PERSONAL_ID && book.is_valid())
}

/// 键盘进程自己记下的生词本，并进这个目录的生词本。
///
/// 鸿蒙从 API 12 起输入法扩展用独立沙箱，键盘写不到设置应用的目录：键盘把「加入生词本」写进自己沙箱里的一本，经共享沙箱镜像发布出来，设置应用导入时调用这里。`published` 是键盘这次发布的那本（`Wordbook` 的 JSON 文件），`previous` 是设置应用存下的、上次已经并过的那一版，没有时为 `None`。只并进上次以后新出现或改过的词条：键盘还没把上次交过的词清走时，用户在设置这边移出的词不会因为键盘那本里还留着而再回来。`previous` 读不懂时当作没有。键盘清走交过的词以后（[`drain_published`]），宿主不再传 `previous`，这时键盘那本里的每个词都是之后加的，全部并进来。
pub fn merge_published(
    directory: &Path,
    published: &Path,
    previous: Option<&Path>,
) -> Result<PersonalImportReport, WordbookLibraryError> {
    let book = read_published(published).ok_or(WordbookLibraryError::InvalidWordbook)?;
    let before = previous
        .and_then(read_published)
        .map(|book| book.entries)
        .unwrap_or_default();
    let fresh = book
        .entries
        .into_iter()
        .filter(|entry| !before.contains(entry))
        .collect();
    WordbookLibrary::new(directory).merge_personal(fresh, PersonalImportReport::default())
}

/// 键盘一侧：设置应用确认已经并过 `published` 这一版以后，把这一版里的词从 `directory` 下键盘自己的生词本里清走，返回清走了几个。
///
/// 键盘那本因此只是一个待交的队列：交过的词不再留在键盘沙箱里，生词本的 5000 词上限按设置应用那本算，用户在背单词里移出一个词以后，键盘再加同一条释义时是新加的，会重新交过去。交出去以后又在键盘里改过释义的词条和之后新加的词留着，等下一轮。
pub fn drain_published(directory: &Path, published: &Path) -> Result<usize, WordbookLibraryError> {
    let book = read_published(published).ok_or(WordbookLibraryError::InvalidWordbook)?;
    WordbookLibrary::new(directory).remove_personal_entries(&book.entries)
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

    /// 把键盘那本复制到 `to`，模拟共享沙箱镜像的发布；返回复制后的路径。
    fn publish(keyboard: &Path, to: &Path) -> std::path::PathBuf {
        std::fs::copy(keyboard.join("vocabulary-wordbooks/personal.json"), to).unwrap();
        to.to_path_buf()
    }

    fn settings_words(settings: &Path) -> Vec<String> {
        WordbookLibrary::new(settings)
            .load(PERSONAL_ID)
            .unwrap()
            .map(|book| book.entries.into_iter().map(|entry| entry.word).collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_published_keyboard_book_merges_only_what_is_new_since_last_time() {
        let keyboard = tempfile::tempdir().unwrap();
        let settings = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        add_gloss(keyboard.path(), "面试", "interview", "").unwrap();
        add_gloss(keyboard.path(), "苹果", "apple", "").unwrap();
        let first = publish(keyboard.path(), &shared.path().join("first.json"));
        let report = merge_published(settings.path(), &first, None).unwrap();
        assert_eq!(report.added, 2);

        // 设置这边移出一个词，键盘还没清走交过的词时又加了一个：移出的词不能回来。
        WordbookLibrary::new(settings.path())
            .remove_personal_entry("apple")
            .unwrap();
        add_gloss(keyboard.path(), "香蕉", "banana", "").unwrap();
        let second = publish(keyboard.path(), &shared.path().join("second.json"));
        let report = merge_published(settings.path(), &second, Some(&first)).unwrap();
        assert_eq!((report.added, report.merged), (1, 0));
        assert_eq!(settings_words(settings.path()), vec!["interview", "banana"]);

        // 上一版读不懂或不在时当作没有，照常全部合并，不报错。
        let garbage = shared.path().join("garbage.json");
        std::fs::write(&garbage, "not json").unwrap();
        assert!(merge_published(settings.path(), &second, Some(&garbage)).is_ok());
        assert!(
            merge_published(settings.path(), &second, Some(&shared.path().join("none"))).is_ok()
        );
        std::fs::write(&garbage, "{}").unwrap();
        assert!(matches!(
            merge_published(settings.path(), &garbage, None),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    #[test]
    fn a_word_removed_in_settings_comes_back_when_the_keyboard_adds_it_again() {
        let keyboard = tempfile::tempdir().unwrap();
        let settings = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        add_gloss(keyboard.path(), "苹果", "apple", "").unwrap();
        let first = publish(keyboard.path(), &shared.path().join("first.json"));
        merge_published(settings.path(), &first, None).unwrap();
        WordbookLibrary::new(settings.path())
            .remove_personal_entry("apple")
            .unwrap();

        // 设置应用确认并过这一版以后，键盘清走交过的词，键盘沙箱里不再留着它。
        assert_eq!(drain_published(keyboard.path(), &first).unwrap(), 1);
        assert!(!keyboard
            .path()
            .join("vocabulary-wordbooks/personal.json")
            .exists());

        // 再加同一条释义是新加的，交过去以后回到背单词里。
        let again = add_gloss(keyboard.path(), "苹果", "apple", "").unwrap();
        assert_eq!(again.outcome, PersonalGlossOutcome::Added);
        let second = publish(keyboard.path(), &shared.path().join("second.json"));
        assert_eq!(
            merge_published(settings.path(), &second, None)
                .unwrap()
                .added,
            1
        );
        assert_eq!(settings_words(settings.path()), vec!["apple"]);
    }

    #[test]
    fn draining_keeps_what_changed_after_it_was_published() {
        let keyboard = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        add_gloss(keyboard.path(), "面试", "interview", "").unwrap();
        add_gloss(keyboard.path(), "苹果", "apple", "").unwrap();
        let published = publish(keyboard.path(), &shared.path().join("published.json"));
        // 交出去以后，interview 又并进了一条释义，还加了一个新词。
        add_gloss(keyboard.path(), "面谈", "interview", "").unwrap();
        add_gloss(keyboard.path(), "香蕉", "banana", "").unwrap();
        assert_eq!(drain_published(keyboard.path(), &published).unwrap(), 1);
        assert_eq!(settings_words(keyboard.path()), vec!["interview", "banana"]);
        assert!(matches!(
            drain_published(keyboard.path(), &shared.path().join("none")),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }
}
