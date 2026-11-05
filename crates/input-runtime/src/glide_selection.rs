//! 混合滑行的隐式确认门禁；明确选词和空格选择高亮候选仍走原来的校验。

use crate::{Action, Command};

pub(crate) fn is_implicit_commit(action: &Action) -> bool {
    matches!(
        action,
        Action::Finish
            | Action::FinishOnFocusLoss
            | Action::Punctuation(_)
            | Action::PunctuationAscii(_)
            | Action::Command(
                Command::CommitRaw | Command::CommitRawWithoutLearning | Command::CommitReading
            )
    )
}
