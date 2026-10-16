//! Where a cloud suggestion and an AI suggestion land among the candidates.
//!
//! The reference states both positions: a cloud result that is not already in the local list is inserted as the second item on the first page, and an AI result as the third. This client keeps the cloud seat but puts the first AI result in the last slot of the first page, so its arrival moves none of the earlier candidates (#6888, `.agents/notes/implemented/feature/2026-10-16-ai-candidate-last-slot.md`). The placement cannot be seen without a real candidate list to insert into - which is why this is an example beside the other dictionary probes rather than a CTest.
//!
//! No network: the provider results are handed to the runtime directly, which is the same entry
//! point a real provider's callback uses.
//!
//! usage: online_slots <verified-dictionary-directory>

use msime_engine::host::{prepare_options, Session};
use msime_input_runtime::{Action, Runtime};

/// Candidates per page; the first AI result takes the last of them.
const PAGE_SIZE: u8 = 9;

/// Where the first AI result lands on a first page that showed `page` before it arrived: the last slot, or right after the other rows when the page was not full.
fn ai_seat(page: &[String]) -> usize {
    usize::from(PAGE_SIZE - 1).min(page.len())
}

fn candidates(runtime: &Runtime<Session>) -> Vec<String> {
    runtime
        .view()
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: online_slots <verified-dictionary-directory>")?,
    )?;
    let temporary = tempfile::tempdir()?;
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        temporary.path().join("user").to_str().unwrap(),
        temporary.path().join("cache").to_str().unwrap(),
        "online-slot-probe",
    )?;

    let compose = |runtime: &mut Runtime<Session>| -> Result<(), Box<dyn std::error::Error>> {
        for byte in b"nihao" {
            runtime.dispatch(Action::Character {
                value: *byte,
                shift: false,
            })?;
        }
        Ok(())
    };

    // A cloud suggestion the local dictionary does not have takes the second seat.
    let mut runtime = Runtime::new(Session::new(&options)?, PAGE_SIZE)?;
    runtime.focus(true)?;
    compose(&mut runtime)?;
    let local = candidates(&runtime);
    let query = runtime
        .online_query()?
        .ok_or("the composition is not online-eligible")?;
    assert!(
        query.cloud_eligible,
        "a five-letter pinyin query should be cloud eligible"
    );
    let cloud = "合成云候选";
    assert!(!local.contains(&cloud.to_string()));
    assert!(runtime.apply_online_candidate(&query, cloud, 0)?);
    let with_cloud = candidates(&runtime);
    assert_eq!(
        with_cloud.get(1).map(String::as_str),
        Some(cloud),
        "cloud landed at {:?}",
        with_cloud.iter().position(|text| text == cloud)
    );
    assert_eq!(
        with_cloud.first(),
        local.first(),
        "the local first choice must keep its seat"
    );

    // An AI suggestion takes the last slot of the first page; everything before it stays put.
    let ai = "合成 AI 候选";
    assert!(runtime.apply_online_candidate(&query, ai, 1)?);
    let with_both = candidates(&runtime);
    let seat = ai_seat(&with_cloud);
    assert_eq!(with_both.get(1).map(String::as_str), Some(cloud));
    assert_eq!(
        with_both.get(seat).map(String::as_str),
        Some(ai),
        "AI landed at {:?}",
        with_both.iter().position(|text| text == ai)
    );
    assert_eq!(with_both[..seat], with_cloud[..seat]);

    // On its own it takes the same slot. The reference packs the seats instead - one local, then cloud, then English, then AI, each only when it exists - which put the AI result second here and moved every candidate after it when it arrived.
    let mut alone = Runtime::new(Session::new(&options)?, PAGE_SIZE)?;
    alone.focus(true)?;
    compose(&mut alone)?;
    let query = alone
        .online_query()?
        .ok_or("the composition is not online-eligible")?;
    assert!(alone.apply_online_candidate(&query, ai, 1)?);
    let ai_only = candidates(&alone);
    let seat = ai_seat(&local);
    assert_eq!(
        ai_only.get(seat).map(String::as_str),
        Some(ai),
        "AI alone landed in {ai_only:?}"
    );
    assert_eq!(ai_only[..seat], local[..seat]);
    assert_eq!(
        ai_only.first(),
        local.first(),
        "the local first choice keeps its seat"
    );

    // A cloud result the local list already has is not inserted twice.
    let mut duplicate = Runtime::new(Session::new(&options)?, PAGE_SIZE)?;
    duplicate.focus(true)?;
    compose(&mut duplicate)?;
    let query = duplicate
        .online_query()?
        .ok_or("the composition is not online-eligible")?;
    let existing = candidates(&duplicate)[0].clone();
    duplicate.apply_online_candidate(&query, &existing, 0)?;
    let after = candidates(&duplicate);
    assert_eq!(
        after.iter().filter(|text| **text == existing).count(),
        1,
        "the existing candidate was duplicated: {after:?}"
    );

    println!("online slots: cloud takes the second seat, AI takes the last slot of the first page without moving the candidates before it, and a duplicate of a local candidate is not inserted");
    Ok(())
}
