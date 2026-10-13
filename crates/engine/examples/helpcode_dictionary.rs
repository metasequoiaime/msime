//! Check helpcode filtering against the pinned dictionaries.
//!
//! The rules come from the Windows source's own documentation, with its worked example: 阿 carries
//! the 自然码 helpcode `ek`. A single trailing code reorders - matching candidates come first and
//! the rest stay - while two codes filter strictly, keeping only what matches. For a phrase the
//! first code is the leading character's first code and the second is the final character's.
//!
//! Helpcode tables are a separate Engine asset from the dictionaries, under `helpcodes/`. A
//! resource directory staged only from `resources/desktop-dictionary.lock.json` does not have them,
//! and without them there is nothing to match: single codes stop reordering and double codes filter
//! everything away, which reads as the feature being broken rather than absent. This probe
//! therefore composes a resource view that has both, and says so if it cannot.
use msime_engine::host::{prepare_options, Session};

const SCHEMA: &str = "ziranma";

/// 构造带有 `helpcodes/` 的资源视图，百兆级词库通过链接复用。
fn resource_view(
    resources: &std::path::Path,
    helpcodes: &std::path::Path,
) -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let view = tempfile::tempdir()?;
    for entry in std::fs::read_dir(resources)? {
        let entry = entry?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(entry.path(), view.path().join(entry.file_name()))?;
        #[cfg(windows)]
        if entry.path().is_dir() {
            std::os::windows::fs::symlink_dir(entry.path(), view.path().join(entry.file_name()))?;
        } else {
            std::os::windows::fs::symlink_file(entry.path(), view.path().join(entry.file_name()))?;
        }
    }
    if !resources.join("helpcodes").is_dir() {
        // Copied rather than linked: the Engine stages its resources into the user directory and
        // does not follow a linked directory through that step, so a link here reads as no tables
        // at all - which looks exactly like the feature not working.
        let target = view.path().join("helpcodes");
        std::fs::create_dir(&target)?;
        for entry in std::fs::read_dir(helpcodes)? {
            let entry = entry?;
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(view)
}

fn candidates(
    view: &std::path::Path,
    slot: usize,
    keys: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut options = prepare_options(
        view.to_str().ok_or("resource path is not UTF-8")?,
        temporary
            .path()
            .join(format!("user-{slot}"))
            .to_str()
            .unwrap(),
        temporary
            .path()
            .join(format!("cache-{slot}"))
            .to_str()
            .unwrap(),
        "helpcode-fixture",
    )?;
    options.learning = false;
    options.helpcode = true;
    options.helpcode_schema = SCHEMA.into();
    let mut session = Session::new(&options)?;
    for character in keys.bytes() {
        session.character(character, false)?;
    }
    Ok(session.snapshot()?.candidates)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let resources = std::fs::canonicalize(
        arguments
            .next()
            .ok_or("usage: helpcode_dictionary <verified-resources> [helpcode-directory]")?,
    )?;
    let helpcodes = match arguments.next() {
        Some(path) => std::fs::canonicalize(path)?,
        // The tables msime carries in the repository, wherever the example is run from.
        None => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/helpcodes"),
    };
    if !resources.join("helpcodes").is_dir() && !helpcodes.is_dir() {
        return Err(format!(
            "no helpcode tables: {} has no helpcodes/ and {} is not a directory",
            resources.display(),
            helpcodes.display()
        )
        .into());
    }
    let view = resource_view(&resources, &helpcodes)?;
    let view = view.path();

    // One code reorders. 阿 leads once `E` is appended, and the candidates that were there without
    // it are still offered - that is what separates this from the two-code case below.
    let plain = candidates(view, 1, "a")?;
    let single = candidates(view, 2, "aE")?;
    assert_eq!(
        single.first().map(String::as_str),
        Some("阿"),
        "aE: {single:?}"
    );
    assert!(
        plain
            .iter()
            .any(|word| word != "阿" && single.contains(word)),
        "aE dropped the candidates that a offered: {plain:?} -> {single:?}"
    );

    // Two codes filter strictly: `EK` is 阿's own pair, so 阿 is all that survives.
    let double = candidates(view, 3, "aEK")?;
    assert_eq!(double, vec!["阿".to_string()], "aEK: {double:?}");

    // A phrase takes its codes from the ends. 阿姨 is 阿(ek) + 姨(ny), so `EN` keeps it; 阿姨好 keeps
    // it too, because 好 is nz and the rule reads the final character's *first* code.
    let phrase = candidates(view, 4, "ayiEN")?;
    assert!(
        phrase.contains(&"阿姨".to_string()),
        "ayiEN dropped 阿姨: {phrase:?}"
    );
    assert!(
        phrase.iter().all(|word| word.starts_with('阿')),
        "ayiEN kept something whose first character is not 阿: {phrase:?}"
    );
    let unfiltered = candidates(view, 5, "ayi")?;
    assert!(
        phrase.len() < unfiltered.len(),
        "ayiEN filtered nothing: {} of {}",
        phrase.len(),
        unfiltered.len()
    );

    println!("helpcodes reorder with one code and filter with two");
    Ok(())
}
