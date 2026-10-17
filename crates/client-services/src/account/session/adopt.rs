//! 采用另一个沙箱发布的会话（#8201 的 P3，`msime_client_account` 的 `session_adopt`）。鸿蒙的设置应用和键盘各有一个沙箱、各存一份会话，靠共享目录互相镜像；镜像以前在 ArkTS 里直接覆盖文件，退出登录、换号和刷新都可能被另一边的旧副本盖回去。这里在本目录的 `account-refresh.lock` 里决定要不要用发布的那一份，与本进程和同目录其他进程的刷新互斥，不联网。
//!
//! 规则只看 `user.id` 和 `expires_at`，不看 `session_id`：
//!
//! - 同一个 `user.id`：`expires_at` 更晚的胜，两个方向都一样，相同时本地不动。`expires_at` 是拿到令牌的时刻加服务端给的固定时长（msime-cloud 的访问令牌 15 分钟），两个沙箱用的是同一台设备的时钟，所以后签发的会话一定过期得更晚：同一个会话族里刷新过的那份胜过刷新前的（用过的刷新令牌不会被留下或换回来，否则下一次刷新被服务端当成重放、超过 30 秒宽限时整个会话族被吊销），同一个账号重新登录的那份胜过上一次登录留下的。
//! - 不同的 `user.id`（换号）：设置应用发布的（`from: settings`）直接替换本地，键盘发布的（`from: keyboard`）一律不用，设置应用里的换号不会被键盘的旧副本撤销。
//! - 发布方没有会话：登录会话在 `from: settings` 时在锁里清掉本地（与 `forget` 相同），`from: keyboard` 时不动；匿名会话一律不动，匿名账号没有退出登录，清掉只会逼着重新注册、白白用掉每个 IP 每天 5 次的注册额度。
//!
//! 以前按 `session_id` 判断「是不是同一次登录」，只在同一次登录内部比 `expires_at`。升级窗口里两边的 `session_id` 来历各不相同（ArkTS 写的扁平登录会话没有，Rust 注册的匿名会话是 v4、旧桥接写回时又丢了，各沙箱第一次认证前各自补上），为了认出同一个会话族叠了好几层特例，仍有漏网的组合会把用过的刷新令牌换回来。只比 `expires_at` 对这些组合一视同仁。

use super::*;

/// 发布这份会话的一方。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionAdoptSource {
    /// 设置应用发布、键盘导入：登录、退出和换号都在设置应用里做，换号以它为准。
    Settings,
    /// 键盘发布、设置应用导入：键盘只会刷新，只采用同一个账号里更新的那一份。
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
    /// 设置应用那边没有登录会话，本地的登录会话已经清掉。
    Cleared,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 登录会话（`account-session.json`）用：按模块说明的规则决定是否采用 `published`（`None` 是发布方没有这份会话），整个过程持有存储的刷新锁，与刷新、登录、退出互斥。`from: Settings` 且 `published` 是 `None` 时清掉本地会话（与 [`Self::forget`] 相同，不联网）。
    ///
    /// 本地没有会话或读不了时，`Settings` 发布的直接替换，`Keyboard` 发布的不用。`published` 不合法时是 [`AccountError::Storage`]，本地不动。采用的会话与原来的不是同一次登录（`user.id` 或 `session_id` 不同）时推进会话代次，进行中的操作和刷新不会再把旧会话写回来。
    pub fn adopt_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, true)
    }

    /// 匿名会话（`anonymous-session.json`）用：与 [`Self::adopt_session`] 相同，只是发布方没有会话时一律不动、不清本地（匿名账号没有退出登录，见模块说明）。
    pub fn adopt_anonymous_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, false)
    }

    fn adopt_with(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
        clear_when_absent: bool,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        if let Some(published) = &published {
            validate_saved_session(published)?;
        }
        // 与登录、刷新的加锁顺序相同：先存储的锁，再会话的互斥锁。
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            let local = self.stored();
            let Some(published) = published else {
                if !(clear_when_absent && from == SessionAdoptSource::Settings) {
                    return Ok(SessionAdoptOutcome::Unchanged);
                }
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
            };
            let adopt = match &local {
                Ok(Some(local)) if local.tokens.user.id == published.tokens.user.id => {
                    published.expires_at_unix_ms > local.expires_at_unix_ms
                }
                // 换号，或者本地没有会话、读不了：只认设置应用发布的。
                _ => from == SessionAdoptSource::Settings,
            };
            if !adopt {
                return Ok(SessionAdoptOutcome::Unchanged);
            }
            let changed = !same_login_option(state.saved.as_ref(), Some(&published));
            self.storage.save(&published)?;
            if state.loaded && changed {
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
