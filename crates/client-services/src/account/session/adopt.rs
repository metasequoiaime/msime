//! 采用另一个沙箱发布的会话（#8201 的 P3，`msime_client_account` 的 `session_adopt`）。鸿蒙的设置应用和键盘各有一个沙箱、各存一份会话，靠共享目录互相镜像；镜像以前在 ArkTS 里直接覆盖文件，退出登录、换号和刷新都可能被另一边的旧副本盖回去。这里在本目录的 `account-refresh.lock` 里决定要不要用发布的那一份，与本进程和同目录其他进程的刷新互斥。
//!
//! 规则只看「是不是同一次登录」：同一个 `user.id` 且两边都有、并且相同的 `session_id`。只有同一次登录内部才按 `expires_at` 新者胜，刷新会推后它；不是同一次登录时，设置应用发布的为准（换号、新登录），键盘发布的一律不用（设置应用里的退出和换号不会被键盘的旧副本撤销）。
//!
//! 比较之前，两边还没有 `session_id` 的旧会话（ArkTS 写的扁平登录会话和匿名会话都没有）先按 `ensure_session_id` 的同一规则补上，由刷新令牌派生（`legacy_session_id`）。升级后两个沙箱是同一份旧会话，一边刷新过（ID 是刷新前那枚令牌派生的），另一边还没被 Rust 写过，补出来的 ID 相同，才会按新者胜采用刷新后的那份；否则会换回已经用过的刷新令牌，下一次刷新被服务端当成重放、整个会话被吊销。

use super::*;

/// 发布这份会话的一方。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionAdoptSource {
    /// 设置应用发布、键盘导入：登录、退出和换号都在设置应用里做，以它为准。
    Settings,
    /// 键盘发布、设置应用导入：键盘只会刷新，只采用同一次登录里更新的那一份。
    Keyboard,
}

/// [`BackendAccountSession::adopt_session`] 做了什么。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAdoptOutcome {
    /// 发布的会话替换了本地的会话。
    Adopted,
    /// 本地的会话（或没有会话）保持原样。
    Unchanged,
    /// 设置应用那边没有会话，本地的会话已经清掉。
    Cleared,
}

/// 没有 `session_id` 的旧会话按刷新令牌补上，与 `ensure_session_id` 写进文件的相同。
fn with_login_id(mut session: SavedAccountSession) -> SavedAccountSession {
    if session.session_id.is_none() {
        session.session_id = Some(legacy_session_id(&session.tokens.refresh_token));
    }
    session
}

/// 两份会话是不是同一次登录。`session_id` 一方没有时不算；调用方先用 [`with_login_id`] 给旧会话补上。
fn same_sign_in(a: &SavedAccountSession, b: &SavedAccountSession) -> bool {
    a.tokens.user.id == b.tokens.user.id && a.session_id.is_some() && a.session_id == b.session_id
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 按 `from` 的规则决定是否采用 `published`（`None` 是发布方没有这份会话），整个过程持有存储的刷新锁，与刷新、登录、退出互斥：
    ///
    /// - `Settings`：`published` 是 `None` 时清掉本地会话（与 [`Self::forget`] 相同，不联网）；本地是同一次登录时按 `expires_at` 新者胜；其余情况（换号、新登录、本地没有或读不了）直接替换。
    /// - `Keyboard`：只在本地是同一次登录、并且 `published` 的 `expires_at` 更晚时采用；其余情况一律不动。
    ///
    /// 两边没有 `session_id` 的旧会话先按刷新令牌补上再比较（见模块说明），采用时写进文件的也是补上 ID 的那份。`published` 不合法时是 [`AccountError::Storage`]，本地不动。登录换了（包括清掉）时推进会话代次，进行中的操作和刷新不会再把旧会话写回来。
    pub fn adopt_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        if let Some(published) = &published {
            validate_saved_session(published)?;
        }
        let published = published.map(with_login_id);
        // 与登录、刷新的加锁顺序相同：先存储的锁，再会话的互斥锁。
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            let local = self.stored().map(|local| local.map(with_login_id));
            let same = |published: &SavedAccountSession| {
                matches!(&local, Ok(Some(local)) if same_sign_in(local, published))
            };
            let newer = |published: &SavedAccountSession| {
                matches!(&local, Ok(Some(local)) if published.expires_at_unix_ms > local.expires_at_unix_ms)
            };
            let adopt = match (from, &published) {
                (SessionAdoptSource::Settings, None) => {
                    let had_session = !matches!(local, Ok(None)) || state.saved.is_some();
                    self.storage.clear()?;
                    if had_session {
                        state.generation = state.generation.saturating_add(1);
                        state.refresh = None;
                    }
                    state.saved = None;
                    state.loaded = true;
                    return Ok(if had_session {
                        SessionAdoptOutcome::Cleared
                    } else {
                        SessionAdoptOutcome::Unchanged
                    });
                }
                (SessionAdoptSource::Settings, Some(published)) => {
                    !same(published) || newer(published)
                }
                (SessionAdoptSource::Keyboard, None) => false,
                (SessionAdoptSource::Keyboard, Some(published)) => {
                    same(published) && newer(published)
                }
            };
            let Some(published) = published.filter(|_| adopt) else {
                return Ok(SessionAdoptOutcome::Unchanged);
            };
            self.storage.save(&published)?;
            let previous = state.saved.clone().map(with_login_id);
            if state.loaded && !same_login_option(previous.as_ref(), Some(&published)) {
                state.generation = state.generation.saturating_add(1);
                state.refresh = None;
            }
            state.saved = Some(published);
            state.loaded = true;
            Ok(SessionAdoptOutcome::Adopted)
        })
    }
}

#[cfg(test)]
mod tests;
