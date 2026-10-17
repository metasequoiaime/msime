//! 采用另一个沙箱发布的会话（#8201 的 P3，`msime_client_account` 的 `session_adopt`）。鸿蒙的设置应用和键盘各有一个沙箱、各存一份会话，靠共享目录互相镜像；镜像以前在 ArkTS 里直接覆盖文件，退出登录、换号和刷新都可能被另一边的旧副本盖回去。这里在本目录的 `account-refresh.lock` 里决定要不要用发布的那一份，与本进程和同目录其他进程的刷新互斥。
//!
//! 规则只看「是不是同一次登录」：同一个 `user.id`，并且两边都有、相同的 `session_id`。只有同一次登录内部才按 `expires_at` 新者胜，刷新会推后它；不是同一次登录时，设置应用发布的为准（换号、新登录），键盘发布的一律不用（设置应用里的退出和换号不会被键盘的旧副本撤销）。
//!
//! 升级窗口是例外。ArkTS 写的扁平登录会话和匿名会话都没有 `session_id`，Rust 第一次用它时按刷新令牌补上派生的 ID（`legacy_session_id`，UUID v8），随后的刷新不改 ID；Rust 自己登录、注册匿名会话给的是随机的 UUID v4。所以一边磁盘上还没有 `session_id`（这个沙箱的会话还没被 Rust 补过 ID，只可能是升级前那次登录），另一边也没有、或者是派生的 ID 时，两份都是升级前那次登录的后代，同一个 `user.id` 就算同一次登录，两个方向都按新者胜。不能要求两边的刷新令牌相同：旧版里键盘会刷新并发布会话，设置应用只在回到前台时导入，访问令牌 15 分钟就过期，升级时设置应用手里往往是键盘早已轮换掉的令牌，键盘手里是新的（多半还已经被 Rust 补上了由它派生的 ID）。这时若不算同一次登录，设置应用会留着用过的令牌、键盘会被换回它，下一次刷新被服务端当成重放（宽限只有 30 秒），整个会话被吊销、两个沙箱一起登出。两边都已经有 `session_id` 时只认相同的 ID。
//!
//! 匿名会话（`adopt_anonymous_session`）放得更宽：同一个 `user.id`，只要有一边是升级前那次登录的后代（没有 `session_id` 或是派生的 v8），就算同一次登录；两边都是 Rust 给的 v4 时才要求相同。匿名会话从来都是 Rust 注册的（v4），旧版的 ArkTS 桥接刷新后按 `anonymousSessionForStorage` 写回时丢掉了 `session_id`，所以升级时常见的是一边还是注册时的 `{R0, v4}`（R0 早已被另一边轮换掉），另一边是 `{R1, 无 ID}`、或者被 Rust 用过以后的 `{R2, H(R1)}`。按登录会话的规则这不算同一次登录：设置应用会留着用过的 R0，键盘会被换回 R0，下一次刷新被服务端当成重放、两个沙箱一起登出。匿名身份是这台设备固定的，用户不会在设置应用里换号；Rust 只在会话读不了或者服务端拒绝刷新、要求重新登录时才再登录一次，那一份是刚签发的，`expires_at` 比已经刷新不了的旧会话晚，新者胜仍是它。

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

/// 升级前那次登录的后代：磁盘上还没有 `session_id`，或者是 [`legacy_session_id`] 派生的（UUID v8）。
fn from_before_the_upgrade(session: &SavedAccountSession) -> bool {
    session
        .session_id
        .is_none_or(|id| id.get_version() == Some(uuid::Version::Custom))
}

/// 两份会话（按磁盘上的原样，不补 ID）是不是同一次登录：同一个 `user.id`，两边都有 `session_id` 时要相同；有一边还没有时，另一边也须是升级前那次登录的后代（见模块说明）。
fn same_sign_in(a: &SavedAccountSession, b: &SavedAccountSession) -> bool {
    a.tokens.user.id == b.tokens.user.id
        && match (a.session_id, b.session_id) {
            (Some(a), Some(b)) => a == b,
            _ => from_before_the_upgrade(a) && from_before_the_upgrade(b),
        }
}

/// 匿名会话的「同一次登录」：同一个 `user.id`，有一边是升级前那次登录的后代时就算，两边都是 Rust 给的 v4 时要相同（见模块说明）。比 [`same_sign_in`] 宽，它算同一次登录的这里都算。
fn same_anonymous_sign_in(a: &SavedAccountSession, b: &SavedAccountSession) -> bool {
    a.tokens.user.id == b.tokens.user.id
        && (from_before_the_upgrade(a)
            || from_before_the_upgrade(b)
            || a.session_id == b.session_id)
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 登录会话（`account-session.json`）用：按 `from` 的规则决定是否采用 `published`（`None` 是发布方没有这份会话），整个过程持有存储的刷新锁，与刷新、登录、退出互斥：
    ///
    /// - `Settings`：`published` 是 `None` 时清掉本地会话（与 [`Self::forget`] 相同，不联网）；本地是同一次登录时按 `expires_at` 新者胜；其余情况（换号、新登录、本地没有或读不了）直接替换。
    /// - `Keyboard`：只在本地是同一次登录、并且 `published` 的 `expires_at` 更晚时采用；其余情况一律不动。
    ///
    /// 一边还没有 `session_id` 时怎样算同一次登录见模块说明；采用没有 `session_id` 的会话时，写进文件的是按刷新令牌补上 ID 的那份。`published` 不合法时是 [`AccountError::Storage`]，本地不动。登录换了（包括清掉）时推进会话代次，进行中的操作和刷新不会再把旧会话写回来。
    pub fn adopt_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, same_sign_in)
    }

    /// 匿名会话（`anonymous-session.json`）用：与 [`Self::adopt_session`] 相同，只是「同一次登录」按 [`same_anonymous_sign_in`] 判断，升级前 ArkTS 写回的没有 `session_id` 的副本（及其派生 v8 的后代）与 Rust 注册时的 v4 副本算同一次登录（见模块说明）。
    pub fn adopt_anonymous_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, same_anonymous_sign_in)
    }

    fn adopt_with(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
        same_sign_in_as: fn(&SavedAccountSession, &SavedAccountSession) -> bool,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        if let Some(published) = &published {
            validate_saved_session(published)?;
        }
        // 与登录、刷新的加锁顺序相同：先存储的锁，再会话的互斥锁。
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            let local = self.stored();
            let same = |published: &SavedAccountSession| {
                matches!(&local, Ok(Some(local)) if same_sign_in_as(local, published))
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
            // 代次按登录会话的规则判断（两种规则里更严的那个）：匿名会话按宽的规则采用了另一份会话族时，进行中的刷新也不会把旧的那份写回来。
            let changed = !matches!(&state.saved, Some(previous) if same_sign_in(previous, &published));
            let published = with_login_id(published);
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
