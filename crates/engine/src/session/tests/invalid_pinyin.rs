use super::{type_text, Fixture};
use crate::types::{Command, SchemeType};

/// #11035：无法切成音节的全拼原文仍要留在组字区，并能用回车原样上屏；下一次组字也不能被前一次卡住。
#[test]
fn invalid_quanpin_input_stays_editable_and_commits_raw() {
    let fixture = Fixture::new("");
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Quanpin);

    type_text(&mut session, "wi");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "wi");
    assert_eq!(snapshot.preedit, "wi");
    assert!(snapshot.candidates.is_empty());

    let result = session.command(Command::CommitRaw);
    assert_eq!(result.commit.as_deref(), Some("wi"));
    assert!(session.snapshot().editing_text.is_empty());

    type_text(&mut session, "ni");
    assert_eq!(session.snapshot().editing_text, "ni");
}
