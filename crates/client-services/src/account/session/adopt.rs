//! 采用另一个沙箱发布的会话（#8201 的 P3，`msime_client_account` 的 `session_adopt`）。鸿蒙的设置应用和键盘各有一个沙箱、各存一份会话，靠共享目录互相镜像；镜像以前在 ArkTS 里直接覆盖文件，退出登录、换号和刷新都可能被另一边的旧副本盖回去。这里在本目录的 `account-refresh.lock` 里决定要不要用发布的那一份，与本进程和同目录其他进程的刷新互斥，不联网。
//!
//! 规则只看 `user.id` 和 `expires_at`，不看 `session_id`：
//!
//! - 同一个 `user.id`：`expires_at` 更晚的胜，两个方向都一样，相同时本地不动。`expires_at` 是拿到令牌的时刻加服务端给的固定时长（msime-cloud 的访问令牌 15 分钟），两个沙箱用的是同一台设备的时钟，所以后签发的会话一定过期得更晚：同一个会话族里刷新过的那份胜过刷新前的（用过的刷新令牌不会被留下或换回来，否则下一次刷新被服务端当成重放、超过 30 秒宽限时整个会话族被吊销），同一个账号重新登录的那份胜过上一次登录留下的。
//! - 不同的 `user.id`（换号）：设置应用发布的（`from: settings`）直接替换本地，键盘发布的（`from: keyboard`）一律不用，设置应用里的换号不会被键盘的旧副本撤销。
//! - 本地没有会话（没有或读不了）：登录会话与换号相同；匿名会话两个方向都采用。用户没打开过设置应用就先启用了键盘时，键盘已经注册了本机的匿名账号，设置应用第一次启动时接过来用，不再注册第二个、多用掉一次每个 IP 每天 5 次的注册额度。
//! - 发布方没有会话：登录会话在 `from: settings` 时在锁里清掉本地（与 `forget` 相同），`from: keyboard` 时不动；匿名会话一律不动，匿名账号没有退出登录，清掉只会逼着重新注册、白白用掉每个 IP 每天 5 次的注册额度。
//! - 匿名账号的身份跟着匿名会话走（[`BackendAccountSession::adopt_anonymous_session_with_identity`]）：采用了发布的匿名会话时，同一把锁里把本地的 `anonymous-account.json` 换成发布方的；发布方没有身份而采用的会话换了账号时删掉本地身份。否则设置应用采用了键盘注册的匿名账号 K、手里却留着自己没注册成的身份 S，K 的会话失效以后会拿 S 登录、换成另一个账号，K 名下的语音样本在需要重新登录才能删时也删不掉。
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
    /// 本地的会话读不了，发布的这份按规则不用（只有登录会话、`from: Keyboard` 会这样），本地保持原样。与 [`Self::Unchanged`] 不同的是这个结论取决于本地此刻读不读得了，不只取决于发布的这一份：本地恢复可读以后，同一份发布物可能就该采用了（同一个账号、发布的更新），宿主不能据此认定同一份发布物以后都不用再试。
    LocalUnreadable,
    /// 设置应用那边没有登录会话，本地的登录会话已经清掉。
    Cleared,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 登录会话（`account-session.json`）用：按模块说明的规则决定是否采用 `published`（`None` 是发布方没有这份会话），整个过程持有存储的刷新锁，与刷新、登录、退出互斥。`from: Settings` 且 `published` 是 `None` 时清掉本地会话（与 [`Self::forget`] 相同，不联网）。
    ///
    /// 本地没有会话或读不了时，`Settings` 发布的直接替换，`Keyboard` 发布的不用（匿名会话两个方向都用，见 [`Self::adopt_anonymous_session`]）；读不了而不用时结果是 [`SessionAdoptOutcome::LocalUnreadable`]，不是 `Unchanged`。`published` 不合法时是 [`AccountError::Storage`]，本地不动。采用的会话与刚读到的本地会话或内存里那份不是同一次登录（`user.id` 或 `session_id` 不同）时推进会话代次，不论这个对象读没读过存储，进行中的操作、刷新和登录都不会再把旧会话或自己的会话写上去。
    pub fn adopt_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, None)
    }

    /// 匿名会话（`anonymous-session.json`）用：与 [`Self::adopt_session`] 相同，只有两处不同：发布方没有会话时一律不动、不清本地（匿名账号没有退出登录）；本地没有会话或读不了时 `Keyboard` 发布的也采用（键盘先于设置应用启用时用键盘注册的那个匿名账号），见模块说明。
    pub fn adopt_anonymous_session(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, Some(None))
    }

    /// 与 [`Self::adopt_anonymous_session`] 相同，并且身份跟着会话走：采用了发布的匿名会话时，在同一把锁里、写会话之前把 `directory` 里的 `anonymous-account.json` 换成发布方的身份 `identity`（发布方没有身份而采用的会话换了账号时删掉本地身份，同一个账号时不动），会话和身份始终属于同一个匿名账号，见 [`PublishedAnonymousIdentity`]。没有采用会话时身份不动。`directory` 是这个匿名会话所在的目录。写会话失败时尽力把身份还原成原来的样子。
    pub fn adopt_anonymous_session_with_identity(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
        directory: &Path,
        identity: &PublishedAnonymousIdentity,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        self.adopt_with(published, from, Some(Some((directory, identity))))
    }

    /// `anonymous` 为 `None` 是登录会话；`Some` 是匿名会话，里面是要跟着采用的身份（`None` 是不管身份）。
    fn adopt_with(
        &self,
        published: Option<SavedAccountSession>,
        from: SessionAdoptSource,
        anonymous: Option<Option<(&Path, &PublishedAnonymousIdentity)>>,
    ) -> Result<SessionAdoptOutcome, AccountError> {
        let identity = anonymous.flatten();
        let anonymous = anonymous.is_some();
        if let Some(published) = &published {
            validate_saved_session(published)?;
        }
        // 与登录、刷新的加锁顺序相同：先存储的锁，再会话的互斥锁。
        self.storage.with_refresh_lock(|| {
            let mut state = self.lock()?;
            let local = self.stored();
            let Some(published) = published else {
                if anonymous || from == SessionAdoptSource::Keyboard {
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
                // 本地没有匿名会话（没有或读不了）：两个方向都采用。键盘先于设置应用启用时，设置应用接过键盘注册的匿名账号，不再注册第二个。
                Ok(None) | Err(_) if anonymous => true,
                // 换号，或者本地没有登录会话、读不了：只认设置应用发布的。
                _ => from == SessionAdoptSource::Settings,
            };
            if !adopt {
                return Ok(if local.is_err() {
                    SessionAdoptOutcome::LocalUnreadable
                } else {
                    SessionAdoptOutcome::Unchanged
                });
            }
            let restore = match identity {
                Some((directory, identity)) => {
                    let same_account = matches!(
                        &local,
                        Ok(Some(local)) if local.tokens.user.id == published.tokens.user.id
                    );
                    identity
                        .follow(directory, same_account)?
                        .map(|restore| (directory, restore))
                }
                None => None,
            };
            // 与刚读到的本地会话比，也与内存里那份比（读过存储时）：哪一个不是同一次登录，都有可能有操作拿着它的代次在路上。这个对象还没读过存储时内存里那份是空的，但同一个对象上已经开始的登录照样推进过代次、在等服务端，不推进代次的话它回来就会把采用的换号盖掉。
            let changed = !matches!(&local, Ok(local) if same_login_option(local.as_ref(), Some(&published)))
                || (state.loaded && !same_login_option(state.saved.as_ref(), Some(&published)));
            if let Err(error) = self.storage.save(&published) {
                // 会话没写进去：身份换回原来的，不留下与本地会话不是同一个账号的身份。还原也失败时交回写会话的错误。
                if let Some((directory, restore)) = restore {
                    let _ = restore.restore(directory);
                }
                return Err(error);
            }
            if changed {
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
