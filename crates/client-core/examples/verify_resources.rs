//! Read-only verification of an already downloaded, pinned resource set.
//!
//! `--omit-on-demand` 按发布包的规则校验：按需下载的文件组整体缺席时只校验其余核心文件，并且只输出核心文件名。哪些组可以缺席由 `--platform <os>`（取值同 `std::env::consts::OS`，见 `resources::on_demand_artifacts`）决定：macOS 是日文词典那一组（词典与两份 Mozc 许可文本）和五笔码表；不给 `--platform` 时只有日文词典那一组，Android 和 iOS 的发布包就是这样。
//!
//! `--edition <id>` 按该版本的资源锁（`Edition::resource_set`，版本表 `shared/contracts/editions.json`）校验，而不是 full 的 `resources/desktop-dictionary.lock.json`；目录必须恰好是该版本带的文件。
use msime_client_core::edition::Edition;
use msime_client_core::resources::{
    on_demand_artifacts, ResourceSet, ResourceStore, ON_DEMAND_JAPANESE_ARTIFACTS,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let usage = "usage: verify_resources [--omit-on-demand [--platform <os>]] [--edition <id>] <resource-directory>";
    let mut arguments = std::env::args_os().skip(1).peekable();
    let mut omit_on_demand = false;
    let mut groups: &[&[&str]] = &[&ON_DEMAND_JAPANESE_ARTIFACTS];
    let mut edition = Edition::full();
    loop {
        match arguments.peek().and_then(|argument| argument.to_str()) {
            Some("--omit-on-demand") => {
                arguments.next();
                omit_on_demand = true;
            }
            Some("--platform") => {
                arguments.next();
                let platform = arguments.next().ok_or(usage)?;
                groups = on_demand_artifacts(platform.to_str().ok_or(usage)?);
                if groups.is_empty() {
                    return Err("that platform ships every resource; drop --omit-on-demand".into());
                }
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
        let shipped = specification.as_shipped_in_groups(&directory, groups);
        ResourceStore::new(&directory).verify(&directory, &shipped)?;
        let on_demand: Vec<&str> = groups
            .iter()
            .flat_map(|group| group.iter().copied())
            .collect();
        for artifact in specification.without(&on_demand).artifacts {
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
