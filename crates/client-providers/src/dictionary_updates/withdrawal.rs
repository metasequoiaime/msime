use super::*;

/// 单独落盘，旧客户端仍能读取原有配置与选择；切换自动更新开关也不清除用户跳过的版本。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rollback {
    skipped_version: String,
}

pub(super) fn remember_skipped(state: &Path, tag: &str) -> Result<(), UpdateError> {
    if version(tag).is_none() {
        return Err(UpdateError::Invalid);
    }
    write(
        &root(state).join("rollback.json"),
        &Rollback {
            skipped_version: tag.to_owned(),
        },
    )
}

pub(super) fn skipped_version(state: &Path) -> Result<Option<[u64; 3]>, UpdateError> {
    let path = root(state).join("rollback.json");
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(UpdateError::Storage),
        Ok(_) => {
            let rollback: Rollback = read(&path)?;
            version(&rollback.skipped_version)
                .map(Some)
                .ok_or(UpdateError::Invalid)
        }
    }
}

/// 调用方持有更新锁。只接受 API 明确标成预发布的当前版本，并要求上一版仍在正式列表里。
pub(super) fn restore_previous(
    context: &UpdateContext,
    releases: &[Release],
) -> Result<bool, UpdateError> {
    let state = &context.state_root;
    let Some(active) = selected(state, &context.baseline) else {
        return Ok(false);
    };
    if !releases
        .iter()
        .any(|r| r.tag_name == active.version && !r.draft && r.prerelease)
    {
        return Ok(false);
    }
    let mut installed: Installed = read(&root(state).join("installed.json"))?;
    let Some(previous) = &installed.previous else {
        return Ok(false);
    };
    if validate_selection(previous, &context.baseline).is_err()
        || version(&previous.version) >= version(&active.version)
        || !releases
            .iter()
            .any(|r| r.tag_name == previous.version && !r.draft && !r.prerelease)
    {
        return Ok(false);
    }
    // 不下载修补旧槽；桌面与语言代次都完整校验，缺失或损坏时保留当前选择等待修复版。
    for set in std::iter::once(&previous.desktop).chain(previous.languages.iter()) {
        let directory = directory(state, set)?;
        if msime_path_trust::reject_symlinked_components(&directory).is_err()
            || store(state).verify(&directory, set).is_err()
        {
            return Ok(false);
        }
    }
    std::mem::swap(&mut installed.active, &mut installed.previous);
    write(&root(state).join("installed.json"), &installed)?;
    Ok(true)
}
