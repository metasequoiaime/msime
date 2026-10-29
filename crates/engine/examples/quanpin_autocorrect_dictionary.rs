//! Real-resource regression probe for Windows-parity quanpin autocorrection.
//!
//! Synthetic Engine tests prove graph mechanics, but only the pinned production dictionary can
//! prove that edit-cost tiers win before word frequency and that a corrected head composes with a
//! jianpin tail. Keep each session isolated so candidate learning cannot influence the ordering.
use msime_engine::host::{prepare_options, Session};

fn candidates(
    resources: &std::path::Path,
    temporary: &std::path::Path,
    slot: &str,
    input: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut options = prepare_options(
        resources.to_str().ok_or("resource path is not UTF-8")?,
        temporary.join(format!("user-{slot}")).to_str().unwrap(),
        temporary.join(format!("cache-{slot}")).to_str().unwrap(),
        "quanpin-autocorrect-fixture",
    )?;
    options.autocorrect_transposition = true;
    options.autocorrect_neighbor = true;
    options.helpcode = false;
    options.mixed_english = false;
    options.learning = false;
    let mut session = Session::new(&options)?;
    for byte in input.bytes() {
        session.character(byte, false)?;
    }
    Ok(session.snapshot()?.candidates)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: quanpin_autocorrect_dictionary <verified-resources>")?,
    )?;
    let temporary = tempfile::tempdir()?;

    let gau = candidates(&resources, temporary.path(), "gau", "gau")?;
    let hauzh = candidates(&resources, temporary.path(), "hauzh", "hauzh")?;
    println!(
        "quanpin autocorrection: gau starts with {:?}; hauzh offers 华中 at {:?}",
        gau.first(),
        hauzh.iter().position(|word| word == "华中")
    );
    assert_eq!(
        gau.first().map(String::as_str),
        Some("挂"),
        "the cheaper gau -> gua transposition must lead the costlier gau -> gai neighbor: {gau:?}"
    );
    assert!(
        hauzh.iter().take(12).any(|word| word == "华中"),
        "a corrected head must compose with the jianpin tail (hauzh -> hua'zh): {hauzh:?}"
    );

    // #9903：全辅音输入是简拼，字面简拼的整词排在邻键纠错读法（qjn -> qun/qin、zhr -> zhe）前面，纠错读法留在后面。
    let qjn = candidates(&resources, temporary.path(), "qjn", "qjn")?;
    let zhr = candidates(&resources, temporary.path(), "zhr", "zhr")?;
    let bqng = candidates(&resources, temporary.path(), "bqng", "bqng")?;
    println!(
        "jianpin over autocorrection: qjn {:?}; zhr {:?}; bqng {:?}",
        &qjn[..qjn.len().min(8)],
        &zhr[..zhr.len().min(8)],
        &bqng[..bqng.len().min(8)]
    );
    assert_eq!(
        qjn.first().map(String::as_str),
        Some("前几年"),
        "the literal qjn phrases must lead the qun/qin corrections: {qjn:?}"
    );
    // 首页可能放不下纠错的「这」，所以只要求首位是 zh'r 的两字词。
    assert!(
        zhr.first().is_some_and(|word| word.chars().count() == 2),
        "a zh'r phrase must lead the zhe correction: {zhr:?}"
    );

    Ok(())
}
