//! Read-only verification of an already downloaded, pinned resource set.
//!
//! `--omit-on-demand` 按 macOS 发布包的规则校验：日文词典那一组文件（词典与两份 Mozc 许可文本）整体缺席时只校验其余核心文件，并且只输出核心文件名。
use msime_client_core::resources::{ResourceSet, ResourceStore, MACOS_ON_DEMAND_ARTIFACTS};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: verify_resources [--omit-on-demand] <resource-directory>";
    let mut arguments = std::env::args_os().skip(1).peekable();
    let omit_on_demand = arguments
        .peek()
        .is_some_and(|argument| argument == "--omit-on-demand");
    if omit_on_demand {
        arguments.next();
    }
    let directory = arguments.next().ok_or(usage)?;
    let directory = std::fs::canonicalize(directory)?;
    let specification: ResourceSet = serde_json::from_str(include_str!(
        "../../../resources/desktop-dictionary.lock.json"
    ))?;
    if omit_on_demand {
        let shipped = specification.as_shipped_in(&directory, &MACOS_ON_DEMAND_ARTIFACTS);
        ResourceStore::new(&directory).verify(&directory, &shipped)?;
        for artifact in specification.without(&MACOS_ON_DEMAND_ARTIFACTS).artifacts {
            println!("{}", artifact.name);
        }
        return Ok(());
    }
    ResourceStore::new(&directory).verify(&directory, &specification)?;
    for artifact in specification.artifacts {
        println!("{}", artifact.name);
    }
    Ok(())
}
