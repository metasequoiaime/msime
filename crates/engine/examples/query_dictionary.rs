//! Real dictionary integration probe; temporary user/cache directories are isolated.
use msime_engine::host::{prepare_options, Session};
use msime_engine::CandidateEdge;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: query_dictionary <verified-dictionary-directory>")?;
    let resources = std::fs::canonicalize(resources)?;
    let temporary = tempfile::tempdir()?;
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        temporary.path().join("user").to_str().unwrap(),
        temporary.path().join("cache").to_str().unwrap(),
        "integration-probe",
    )?;
    let mut session = Session::new(&options)?;
    for character in b"nihao" {
        session.character(*character, false)?;
    }
    let snapshot = session.snapshot()?;
    let index = snapshot
        .candidates
        .iter()
        .position(|word| word == "你好")
        .ok_or("expected phrase missing from published dictionary")?;
    let selected = session.select(index)?;
    if !selected.has_commit || selected.commit != "你好" {
        return Err("published dictionary selection failed".into());
    }
    drop(session);
    for (edge, expected) in [
        (CandidateEdge::FirstHan, "你"),
        (CandidateEdge::LastHan, "好"),
    ] {
        let mut session = Session::new(&options)?;
        for character in b"nihao" {
            session.character(*character, false)?;
        }
        let snapshot = session.snapshot()?;
        let index = snapshot
            .candidates
            .iter()
            .position(|word| word == "你好")
            .ok_or("expected edge-selection phrase missing")?;
        let selected = session.select_edge(index, edge)?;
        if !selected.handled
            || !selected.has_commit
            || selected.commit != expected
            || !session.snapshot()?.editing_text.is_empty()
        {
            return Err("published dictionary edge selection failed".into());
        }
    }
    for (profile, code) in [(0, "bk"), (1, "by"), (2, "bg"), (3, "b;")] {
        let mut profile_options = options.clone();
        profile_options.scheme = 1;
        profile_options.shuangpin_profile = profile;
        let mut session = Session::new(&profile_options)?;
        for byte in code.bytes() {
            session.character(byte, false)?;
        }
        if !session
            .snapshot()?
            .candidates
            .iter()
            .any(|word| word == "冰")
        {
            return Err(
                format!("published dictionary shuangpin profile {profile} query failed").into(),
            );
        }
    }
    println!(
        "published dictionary: phrase, Han edges and four shuangpin profiles passed; isolated generation prepared"
    );
    Ok(())
}
