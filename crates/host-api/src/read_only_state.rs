//! 对共享状态目录没有写权限的输入宿主怎么准备运行时（#10945）。
//!
//! iOS 键盘扩展没开「允许完全访问」时，App Group 是只读的：读得到 App 写下的偏好和准备好的代次，任何写都以 `EPERM` 失败。可写的准备（[`crate::prepare_host_configuration_for_edition`]）要建偏好锁、写第一份偏好、在代次里回放日志、写校验标记，于是第一步就失败，键盘打不了字。
//!
//! 这里的准备一个字节也不写共享状态目录：
//!
//! - 偏好照常从共享目录读（`PreferencesStore` 的只读入口在没有写权限时不取锁）。
//! - App 已经准备好本版本的代次、两个租约锁文件也在时，只读地用它（`msime_engine::host::open_prepared_options_for`），用户词库和学到的东西都在；会话只读打开已有的锁文件取共享租约（`DictionaryAccess::try_session_read_only`），App 换代次、删旧代次、维护个人词库仍被它挡住，与有完全访问时相同。
//! - 拿不到这样的代次（App 还没为这个版本准备过、锁文件不在、锁取不了）时用只读布局：词库就是扩展随包的资源，没有用户词库，但能打字；随包资源没有谁会改，用不着租约。
//! - 资源校验标记、缓存和只读布局的用户目录放在宿主自己能写的 `private_root`（扩展自己的容器）。
//!
//! 准备出的文档带 `read_only_state: true`，会话据此不学习、不记选词统计和已载入的代次。

use std::path::Path;

use msime_client_core::edition::Edition;
use msime_client_core::preferences::{Preferences, PreferencesStore};
use msime_client_core::resources::ResourceSet;
use msime_client_services::dictionary::access::DictionaryAccess;
use serde_json::Value;

/// 只读地准备 `state_root` 上的宿主，返回 HostOptions 文档。`resources` 是宿主自己随包的资源，`private_root` 是它自己能写、不在 `state_root` 里的目录。
pub(crate) fn prepare_read_only_host(
    resources: &Path,
    state_root: &Path,
    private_root: &Path,
    edition: &'static Edition,
) -> Result<String, Box<dyn std::error::Error>> {
    let state_root = std::path::absolute(state_root)?;
    let private_root = std::path::absolute(private_root)?;
    if private_root.starts_with(&state_root) || state_root.starts_with(&private_root) {
        return Err("the private root must be outside the state root".into());
    }
    crate::reject_symlinked_state_root(&state_root)?;
    msime_path_trust::reject_symlinked_components(&private_root)?;
    std::fs::create_dir_all(&private_root)?;
    let specification = edition.resource_set()?;
    // App 选中并装好的下载代次：校验过、只读租约也取得到才用，否则退回随包资源。
    if let Some((updated, selected)) =
        crate::dictionary_updates::selected_resources(&state_root, &specification)
    {
        let leased =
            msime_client_providers::dictionary_updates::lease_read_only(&state_root, &updated)
                .is_some();
        if leased {
            if let Ok(mut document) = prepare_on(
                &updated,
                &state_root,
                &private_root,
                &selected,
                &[],
                edition,
            ) {
                crate::attach_bundle_beside_update(
                    &mut document,
                    resources,
                    &state_root,
                    &specification,
                    edition,
                );
                return Ok(serde_json::to_string_pretty(&document)?);
            }
        }
    }
    let document = prepare_on(
        resources,
        &state_root,
        &private_root,
        &specification,
        crate::ON_DEMAND_ARTIFACTS,
        edition,
    )?;
    Ok(serde_json::to_string_pretty(&document)?)
}

/// 在 `resources` 上只读准备：校验资源（标记写在 `private_root`），读偏好，选共享代次或只读布局。
pub(crate) fn prepare_on(
    resources: &Path,
    state_root: &Path,
    private_root: &Path,
    specification: &ResourceSet,
    on_demand: &[&[&str]],
    edition: &'static Edition,
) -> Result<Value, Box<dyn std::error::Error>> {
    let resources = crate::without_verbatim_prefix(std::fs::canonicalize(resources)?);
    crate::verify_resources_once(&resources, specification, private_root, on_demand)?;
    let preferences = read_only_preferences(state_root, edition)?;
    let schemes = crate::engine_schemes(edition);
    let text = |path: &Path| -> Result<String, Box<dyn std::error::Error>> {
        Ok(path.to_str().ok_or("non-UTF-8 runtime path")?.to_owned())
    };
    let shared = msime_engine::host::open_prepared_options_for(
        &text(&resources)?,
        &text(&state_root.join("user"))?,
        &text(&private_root.join("cache"))?,
        &specification.generation()?,
        schemes,
    )
    .map_err(|error| error.to_string())
    .and_then(|prepared| {
        // 锁文件是能写的一方建的；这里只确认它们在、取得了只读的共享锁，马上放掉，会话建的时候再取。
        DictionaryAccess::try_session_read_only(
            Path::new(&prepared.user_data),
            Path::new(&prepared.dictionaries),
        )
        .map(|_| prepared)
        .map_err(|error| format!("dictionary lease unavailable: {}", error.kind()))
    });
    let prepared = match shared {
        Ok(prepared) => prepared,
        Err(reason) => {
            crate::diagnostics::report(
                "read-only runtime on the bundled dictionaries",
                &format!("the shared generation is not usable without write access ({reason}); typing runs on the packaged dictionaries without the user dictionary"),
            );
            msime_engine::host::read_only_layout_options_for(
                &text(&resources)?,
                &text(&private_root.join("user"))?,
                &text(&private_root.join("cache"))?,
                schemes,
            )?
        }
    };
    let mut options =
        crate::prepared_host_options(prepared, &resources, state_root, preferences, edition)?;
    options.read_only_state = Some(true);
    Ok(serde_json::to_value(&options)?)
}

/// 共享目录里的偏好，只读：没有偏好文件时是本版本的默认值，读不了的文件在内存里救下认得的设置，都不写文件。
fn read_only_preferences(
    state_root: &Path,
    edition: &'static Edition,
) -> Result<Preferences, Box<dyn std::error::Error>> {
    let runtime =
        PreferencesStore::for_edition(state_root, edition).load_for_runtime_without_writing()?;
    crate::report_unreadable_preferences(&runtime);
    Ok(runtime.snapshot.preferences)
}
