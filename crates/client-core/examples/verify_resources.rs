//! Read-only verification of an already downloaded, pinned resource set.
//!
//! `--omit-on-demand` 按 macOS 发布包的规则校验：日文词典那一组文件（词典与两份 Mozc 许可文本）整体缺席时只校验其余核心文件，并且只输出核心文件名。
//!
//! `--edition <id>` 按该版本的资源锁（`Edition::resource_set`，版本表 `shared/contracts/editions.json`）校验，而不是 full 的 `resources/desktop-dictionary.lock.json`；目录必须恰好是该版本带的文件。
use msime_client_core::edition::Edition;
use msime_client_core::resources::{ResourceSet, ResourceStore, MACOS_ON_DEMAND_ARTIFACTS};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: verify_resources [--omit-on-demand] [--edition <id>] <resource-directory>";
    let mut arguments = std::env::args_os().skip(1).peekable();
    let mut omit_on_demand = false;
    let mut edition = Edition::full();
    loop {
        match arguments.peek().and_then(|argument| argument.to_str()) {
            Some("--omit-on-demand") => {
                arguments.next();
                omit_on_demand = true;
            }
            Some("--edition") => {
                arguments.next();
                let id = arguments.next().ok_or(usage)?;
                edition = Edition::by_id(id.to_str().ok_or(usage)?).ok_or("unknown edition")?;
            }
            _ => break,
        }
    }
    let directory = arguments.next().ok_or(usage)?;
    let directory = std::fs::canonicalize(directory)?;
    let specification: ResourceSet = edition.resource_set()?;
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
