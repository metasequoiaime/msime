//! Output validation and dispatch, independent of Tauri and Win32 for tests.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMode {
    Tsf,
    SendInput,
    Clipboard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    InvalidText,
    TsfRequiresServer,
    Unavailable,
}

pub fn submit(
    text: &str,
    mode: &str,
    mut deliver: impl FnMut(OutputMode, &str) -> bool,
) -> Result<(), OutputError> {
    if text.is_empty()
        || text.len() > 4096
        || msime_client_core::has_disallowed_control_with_options(text, true)
    {
        return Err(OutputError::InvalidText);
    }
    let mode = match mode {
        "tsf" => OutputMode::Tsf,
        "sendinput" => OutputMode::SendInput,
        "ctrl_v" => OutputMode::Clipboard,
        _ => return Err(OutputError::Unavailable),
    };
    if deliver(mode, text) {
        return Ok(());
    }
    if mode == OutputMode::Tsf {
        return Err(OutputError::TsfRequiresServer);
    }
    Err(OutputError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_are_not_silently_substituted() {
        for (setting, expected) in [
            ("tsf", OutputMode::Tsf),
            ("sendinput", OutputMode::SendInput),
            ("ctrl_v", OutputMode::Clipboard),
        ] {
            let mut calls = 0;
            assert_eq!(
                submit("synthetic transcript", setting, |mode, text| {
                    calls += 1;
                    assert_eq!(mode, expected);
                    assert_eq!(text, "synthetic transcript");
                    true
                }),
                Ok(())
            );
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn invalid_input_never_reaches_the_host() {
        for text in [
            "".to_owned(),
            "x\0y".into(),
            "x\u{1b}y".into(),
            "界".repeat(1366),
        ] {
            assert_eq!(
                submit(&text, "sendinput", |_, _| panic!("must not deliver")),
                Err(OutputError::InvalidText)
            );
        }
        assert_eq!(
            submit("synthetic", "unknown", |_, _| panic!("must not deliver")),
            Err(OutputError::Unavailable)
        );
    }

    #[test]
    fn failures_are_not_reported_as_commits_or_retried() {
        let mut calls = 0;
        assert_eq!(
            submit("synthetic", "ctrl_v", |_, _| {
                calls += 1;
                false
            }),
            Err(OutputError::Unavailable)
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn tsf_failure_is_explicitly_not_a_native_fallback() {
        assert_eq!(
            submit("synthetic", "tsf", |_, _| false),
            Err(OutputError::TsfRequiresServer)
        );
    }

    #[test]
    fn literal_whitespace_and_surrogates_are_preserved() {
        let text = "test\r\n\t🧪";
        assert_eq!(submit(text, "ctrl_v", |_, actual| actual == text), Ok(()));
        assert_eq!(submit(&"x".repeat(4096), "sendinput", |_, _| true), Ok(()));
    }
}
