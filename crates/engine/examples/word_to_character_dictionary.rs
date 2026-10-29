//! Check word-to-character selection against the pinned dictionaries.
//!
//! `[` commits the first han character of the highlighted candidate and `]` the last. The rule only
//! means something where a multi-character candidate exists to take an edge from, so an empty
//! dictionary cannot exercise it at all.
//!
//! The composition is consumed either way. That is not incidental: the Windows source clears its
//! state after sending the commit, so a host that left the rest of the input composing would
//! disagree with it.
use msime_engine::host::{prepare_options, Session};
use msime_engine::CandidateEdge;

struct Probe {
    resources: std::path::PathBuf,
    temporary: tempfile::TempDir,
    sequence: usize,
}

impl Probe {
    fn new(resources: std::path::PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            resources,
            temporary: tempfile::tempdir()?,
            sequence: 0,
        })
    }

    fn typed(&mut self, keys: &str) -> Result<Session, Box<dyn std::error::Error>> {
        self.sequence += 1;
        let slot = self.sequence;
        let mut options = prepare_options(
            self.resources
                .to_str()
                .ok_or("resource path is not UTF-8")?,
            self.temporary
                .path()
                .join(format!("user-{slot}"))
                .to_str()
                .unwrap(),
            self.temporary
                .path()
                .join(format!("cache-{slot}"))
                .to_str()
                .unwrap(),
            "word-character-fixture",
        )?;
        // Nothing here should be learned; this probe is about what a selection commits.
        options.learning = false;
        // As host-api configures every session. Without whole-sentence alternatives the three-character reading of `zhonghuaren` came only from the dropped Google sentence path, and the dictionary's first row is the four-character completion 中华人民.
        options.sentence_alternatives = true;
        // `shi` has to offer its English candidate; `prepare_options` has required five letters since the Windows parity change, after this probe was written.
        options.english_minimum_prefix = 2;
        let mut session = Session::new(&options)?;
        for character in keys.bytes() {
            session.character(character, false)?;
        }
        Ok(session)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: word_to_character_dictionary <verified-resources>")?,
    )?;
    let mut probe = Probe::new(resources)?;

    // Two characters: each edge takes its own end, and the input is used up.
    for (keys, edge, expected) in [
        ("nihao", CandidateEdge::FirstHan, "你"),
        ("nihao", CandidateEdge::LastHan, "好"),
    ] {
        let mut session = probe.typed(keys)?;
        let result = session.select_edge(0, edge)?;
        assert!(result.handled, "{keys} {edge:?} was not handled");
        assert_eq!(result.commit, expected, "{keys} {edge:?}");
        assert!(
            session.snapshot()?.editing_text.is_empty(),
            "{keys} {edge:?} left the composition behind"
        );
    }

    // Three characters, so "last" cannot be satisfied by taking the second one.
    let mut three = probe.typed("zhonghuaren")?;
    let word = three.snapshot()?.candidates[0].clone();
    assert_eq!(
        word.chars().count(),
        3,
        "expected a three-character candidate, got {word}"
    );
    let last = three.select_edge(0, CandidateEdge::LastHan)?;
    assert_eq!(
        last.commit,
        word.chars().last().unwrap().to_string(),
        "last han of {word}"
    );

    // A candidate with no han characters at all - the English candidate this input offers. The
    // Engine declines rather than inventing an edge, and leaves the composition alone so the host
    // can fall back to sending the candidate whole, which is what the Windows source does.
    let mut english = probe.typed("shi")?;
    let snapshot = english.snapshot()?;
    let index = snapshot
        .candidate_sources
        .iter()
        .position(|source| *source != 0)
        .ok_or("this input no longer offers a non-dictionary candidate")?;
    let declined = english.select_edge(index, CandidateEdge::FirstHan)?;
    assert!(
        !declined.handled,
        "a candidate with no han character was handled"
    );
    assert_eq!(
        english.snapshot()?.editing_text,
        "shi",
        "a declined edge selection disturbed the composition"
    );

    // An index past the end is declined rather than throwing or committing something.
    let mut ranged = probe.typed("nihao")?;
    let out_of_range = ranged.select_edge(9999, CandidateEdge::FirstHan)?;
    assert!(!out_of_range.handled, "an out-of-range index was handled");
    assert!(
        out_of_range.commit.is_empty(),
        "an out-of-range index committed text"
    );

    println!("word-to-character takes each edge and consumes the composition");
    Ok(())
}
