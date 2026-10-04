//! Real dictionary integration probe for the nine-key whole-sentence arbitration.
//!
//! The Engine reads `msime-bigram.bin` and `msime-trigram.bin` from the dictionary directory and silently falls
//! back to unweighted lattice paths when they are absent -- candidates still come out, just ordered
//! as if there were no language model. A resource directory staged from an older dictionary release
//! than the locked Engine therefore looks healthy while ranking whole sentences badly, which is
//! exactly how `64426` ended up publishing `米高哦` above `米高`. This probe fails when the tables
//! are missing from the directory, and when their presence makes no difference to the order.
use msime_engine::host::{prepare_options, Session};

const NGRAM_TABLES: [&str; 2] = ["msime-bigram.bin", "msime-trigram.bin"];

fn candidates(
    resources: &std::path::Path,
    keys: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        temporary.path().join("user").to_str().unwrap(),
        temporary.path().join("cache").to_str().unwrap(),
        "nine-key-probe",
    )?;
    let mut session = Session::new(&options)?;
    session.set_nine_key_enabled(true)?;
    for key in keys.bytes() {
        session.character(key, false)?;
    }
    Ok(session.snapshot()?.candidates)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: ninekey_dictionary <verified-dictionary-directory>")?;
    let resources = std::fs::canonicalize(resources)?;
    for table in NGRAM_TABLES {
        if !resources.join(table).is_file() {
            return Err(format!(
                "{table} missing from the staged resources: the dictionary release predates the locked Engine"
            )
            .into());
        }
    }

    // The same directory minus the tables, to prove they are what orders the whole-sentence paths.
    let unweighted = tempfile::tempdir()?;
    for entry in std::fs::read_dir(&resources)? {
        let entry = entry?;
        let name = entry.file_name();
        if NGRAM_TABLES.iter().any(|table| name == *table) {
            continue;
        }
        std::os::unix::fs::symlink(entry.path(), unweighted.path().join(name))?;
    }

    let weighted_order = candidates(&resources, "64426")?;
    let unweighted_order = candidates(unweighted.path(), "64426")?;
    if weighted_order.is_empty() || unweighted_order.is_empty() {
        return Err("nine-key query produced no candidates".into());
    }
    if weighted_order == unweighted_order {
        return Err(
            "the ngram tables did not change the candidate order: they are not being read".into(),
        );
    }
    println!(
        "nine-key whole-sentence arbitration reads the ngram tables: {} then {}",
        weighted_order[..weighted_order.len().min(3)].join(" "),
        unweighted_order[..unweighted_order.len().min(3)].join(" ")
    );
    Ok(())
}
