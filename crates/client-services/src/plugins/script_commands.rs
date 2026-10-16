//! 脚本插件在 `/` 指令里的触发词：哪些插件的哪些指令算进来、它们之间和用户指令表之间谁生效，以及投递时交给插件的随机数。只在桌面编译。
//!
//! - **算进来的插件**（[`granted_commands`]）：授权文件里启用的前 `limits::MAX_ENABLED_SCRIPTS` 条中，用户授予了 `provide_commands`、清单也申请了它、清单能载入的那些，指令按清单原样、按执行顺序。只读清单，不读模块也不算内容哈希：输入法在聚焦时调用它，模块的读取和核对哈希都留在后台编译线程上，触发词因此不等模块编译好就生效（模块还没就绪时投递返回「没运行」，列表里是原样的兜底行）。
//! - **谁生效**（[`resolve`]）：和引擎的 `local::command::usable_plugin_commands` 一一对应。触发词是 2 到 16 个小写字母、标题是 1 到 8 个字符（不含控制字符，首尾不是空白），不等于、也不和内置触发词、用户指令表的触发词、排在前面的生效插件触发词互为前缀。用户指令表永远赢，插件之间按执行顺序先到先得。client-services 不能依赖引擎（`docs/ARCHITECTURE.md`），所以这里另写一份，host-api 的一致性测试拿同一组输入对两边。
//! - 输入法交给引擎的是 [`granted_commands`] 的原样列表，由引擎按上面的规则过滤（用户指令表变了引擎当场重新过滤）；这里的 [`resolve`] 给授权页说明每条指令的状态。
//! - **随机数**（[`random_bytes`]）：取自操作系统的 CSPRNG（`getrandom`），每次投递 64 字节。取不到时不投递，绝不拿全 0 顶替：那会让 UUID 这类输出变得可预测。

use std::path::Path;

use msime_script_runtime::{limits, Capability, CapabilitySet};
use serde::Serialize;

use super::script_grants::ScriptGrants;
use super::script_pack::{
    ScriptCommand, BUILTIN_COMMAND_TRIGGERS, COMMAND_TITLE_CHARS, COMMAND_TRIGGER_BYTES,
};
use super::{command_table, load_package_unread, BuiltinDirs, PluginContent, PluginKind};

/// 一条插件指令在输入法里的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandStatus {
    /// 生效：用户打这个触发词时由这个插件给出候选。
    Active,
    /// 和用户指令表的触发词 `conflict` 相同或互为前缀，归用户的指令。
    ShadowedByUserTable { conflict: String },
    /// 和执行顺序排在前面的插件 `conflict_id` 的触发词 `conflict` 相同或互为前缀，归那个插件。
    ShadowedByPlugin {
        conflict_id: String,
        conflict: String,
    },
}

/// 一条插件指令和它的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedCommand {
    /// 声明它的插件。
    pub id: String,
    pub trigger: String,
    pub title: String,
    #[serde(flatten)]
    pub status: CommandStatus,
}

/// 两个触发词相等或一个是另一个的前缀：用户打着其中一个时，分不清是在打哪一个。同引擎的规则。
fn triggers_overlap(left: &str, right: &str) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn valid_trigger(trigger: &str) -> bool {
    COMMAND_TRIGGER_BYTES.contains(&trigger.len())
        && trigger.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// 引擎的下限：1 到 8 个字符，不含控制字符，首尾不是空白。清单校验更严，载入的清单一定满足它。
fn valid_title(title: &str) -> bool {
    COMMAND_TITLE_CHARS.contains(&title.chars().count())
        && !title.chars().any(char::is_control)
        && title.trim() == title
}

/// 用户指令表里引擎会用的触发词：1 到 32 个小写字母，同引擎的 `usable_command_table`。
fn valid_user_trigger(trigger: &str) -> bool {
    (1..=command_table::MAX_TRIGGER_BYTES).contains(&trigger.len())
        && trigger.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// 按执行顺序排好的插件指令（`plugins` 的顺序就是执行顺序，每个插件的指令按清单顺序）在用户指令表 `user_triggers` 之下各自的状态。触发词或标题不合规、和内置触发词冲突的指令不列出：清单校验已经拒绝了它们，引擎也会丢掉，这里只为和引擎一一对应。
pub fn resolve(
    user_triggers: &[String],
    plugins: &[(String, Vec<ScriptCommand>)],
) -> Vec<ResolvedCommand> {
    let user: Vec<&str> = user_triggers
        .iter()
        .map(String::as_str)
        .filter(|trigger| valid_user_trigger(trigger))
        .collect();
    let mut resolved: Vec<ResolvedCommand> = Vec::new();
    for (id, commands) in plugins {
        for command in commands {
            let trigger = command.trigger.as_str();
            if !valid_trigger(trigger)
                || !valid_title(&command.title)
                || BUILTIN_COMMAND_TRIGGERS
                    .iter()
                    .any(|builtin| triggers_overlap(trigger, builtin))
            {
                continue;
            }
            let status =
                if let Some(conflict) = user.iter().find(|user| triggers_overlap(trigger, user)) {
                    CommandStatus::ShadowedByUserTable {
                        conflict: (*conflict).to_owned(),
                    }
                } else if let Some(earlier) = resolved.iter().find(|earlier| {
                    earlier.status == CommandStatus::Active
                        && triggers_overlap(trigger, &earlier.trigger)
                }) {
                    CommandStatus::ShadowedByPlugin {
                        conflict_id: earlier.id.clone(),
                        conflict: earlier.trigger.clone(),
                    }
                } else {
                    CommandStatus::Active
                };
            resolved.push(ResolvedCommand {
                id: id.clone(),
                trigger: command.trigger.clone(),
                title: command.title.clone(),
                status,
            });
        }
    }
    resolved
}

/// 会运行、并且能提供指令的插件和它们声明的指令，按执行顺序：启用的前 `limits::MAX_ENABLED_SCRIPTS` 条里，用户授予了 `provide_commands`、清单也申请了它、清单能载入的那些。只读清单（`root` 下已安装的，官方的在 `builtin_packs` 下）和模块文件的大小，不读模块、不核对内容哈希，见模块说明。
pub fn granted_commands(
    root: &Path,
    builtin_packs: Option<&Path>,
    grants: &ScriptGrants,
) -> Vec<(String, Vec<ScriptCommand>)> {
    grants
        .enabled()
        .filter_map(|grant| {
            let commands = granted_commands_of(root, builtin_packs, &grant.id, grant.granted);
            (!commands.is_empty()).then(|| (grant.id.clone(), commands))
        })
        .collect()
}

/// 一个插件交给输入法的指令：用户授予了 `provide_commands`（`granted`）、清单也申请了它、清单能载入时是清单里的指令（原样、按清单顺序），否则为空。只读清单，同 [`granted_commands`]；输入法在授权没变、只有包的清单变了时用它重算这一个插件。
pub fn granted_commands_of(
    root: &Path,
    builtin_packs: Option<&Path>,
    id: &str,
    granted: CapabilitySet,
) -> Vec<ScriptCommand> {
    if !granted.contains(Capability::ProvideCommands) {
        return Vec::new();
    }
    let Ok(summary) = load_package_unread(
        root,
        BuiltinDirs::packs(builtin_packs),
        PluginKind::Script,
        id,
    ) else {
        return Vec::new();
    };
    match summary.content {
        PluginContent::Script(pack) if pack.permissions.contains(Capability::ProvideCommands) => {
            pack.commands
        }
        _ => Vec::new(),
    }
}

/// 授权页用的用户指令表触发词：`enabled` 里按顺序启用的指令表包合起来，和输入法交给引擎的是同一份（`command_table::enabled_commands`）。`/` 指令关着时引擎拿到的是空表，授权页仍按启用的表说明，打开 `/` 之后就是那样。
pub fn user_triggers(root: &Path, enabled: &[String]) -> Vec<String> {
    command_table::enabled_commands(root, enabled)
        .into_iter()
        .map(|row| row.trigger)
        .collect()
}

/// 用操作系统的 CSPRNG 填满 `out`。取不到时返回 false，`out` 的内容不可用。
pub fn random_bytes(out: &mut [u8]) -> bool {
    getrandom::fill(out).is_ok()
}

// 一次投递交给插件的随机数就是 `limits::RANDOM_BYTES_PER_EVENT` 字节，宿主照它开缓冲区。
const _: () = assert!(limits::RANDOM_BYTES_PER_EVENT == 64);

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(pairs: &[(&str, &str)]) -> Vec<ScriptCommand> {
        pairs
            .iter()
            .map(|(trigger, title)| ScriptCommand {
                trigger: (*trigger).to_owned(),
                title: (*title).to_owned(),
            })
            .collect()
    }

    fn statuses(resolved: &[ResolvedCommand]) -> Vec<(&str, &str, CommandStatus)> {
        resolved
            .iter()
            .map(|command| {
                (
                    command.id.as_str(),
                    command.trigger.as_str(),
                    command.status.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn the_user_table_always_wins_and_plugins_are_first_come_first_served() {
        let plugins = vec![
            (
                "first".to_owned(),
                commands(&[("uuid", "UUID"), ("guid", "GUID")]),
            ),
            (
                "second".to_owned(),
                commands(&[("uuidx", "长"), ("abc", "字母")]),
            ),
            (
                "third".to_owned(),
                commands(&[("guid", "又一个"), ("ab", "短")]),
            ),
        ];
        let resolved = resolve(&["gu".to_owned()], &plugins);
        assert_eq!(
            statuses(&resolved),
            [
                ("first", "uuid", CommandStatus::Active),
                (
                    "first",
                    "guid",
                    CommandStatus::ShadowedByUserTable {
                        conflict: "gu".into()
                    }
                ),
                (
                    "second",
                    "uuidx",
                    CommandStatus::ShadowedByPlugin {
                        conflict_id: "first".into(),
                        conflict: "uuid".into()
                    }
                ),
                ("second", "abc", CommandStatus::Active),
                (
                    "third",
                    "guid",
                    CommandStatus::ShadowedByUserTable {
                        conflict: "gu".into()
                    }
                ),
                (
                    "third",
                    "ab",
                    CommandStatus::ShadowedByPlugin {
                        conflict_id: "second".into(),
                        conflict: "abc".into()
                    }
                ),
            ]
        );
    }

    /// 被用户指令表挡住的插件指令不挡后面的插件：比的是排在前面、已经生效的那些，同引擎。
    #[test]
    fn a_command_the_user_table_shadows_does_not_shadow_later_plugins() {
        let plugins = vec![
            ("first".to_owned(), commands(&[("uuid", "UUID")])),
            ("second".to_owned(), commands(&[("uuidb", "乙")])),
        ];
        let resolved = resolve(&["uuida".to_owned()], &plugins);
        assert_eq!(
            statuses(&resolved),
            [
                (
                    "first",
                    "uuid",
                    CommandStatus::ShadowedByUserTable {
                        conflict: "uuida".into()
                    }
                ),
                ("second", "uuidb", CommandStatus::Active),
            ]
        );
    }

    #[test]
    fn equal_and_prefix_triggers_conflict_both_ways() {
        for (user, trigger) in [("uuid", "uuid"), ("uu", "uuid"), ("uuidv", "uuid")] {
            let resolved = resolve(
                &[user.to_owned()],
                &[("p".to_owned(), commands(&[(trigger, "T")]))],
            );
            assert_eq!(
                resolved[0].status,
                CommandStatus::ShadowedByUserTable {
                    conflict: user.into()
                },
                "{user} {trigger}"
            );
        }
        // 用户表里引擎不用的触发词（大写、超长）不挡插件。
        let resolved = resolve(
            &["UU".to_owned(), "u".repeat(33)],
            &[("p".to_owned(), commands(&[("uuid", "T")]))],
        );
        assert_eq!(resolved[0].status, CommandStatus::Active);
    }

    #[test]
    fn commands_the_engine_would_drop_are_not_listed() {
        let plugins = vec![(
            "p".to_owned(),
            commands(&[
                ("u", "单字母"),
                ("abcdefghijklmnopq", "十七个"),
                ("Uuid", "大写"),
                ("rqx", "内置前缀"),
                ("f", "内置前缀"),
                ("ok", ""),
                ("okay", "九个字九个字九个字"),
                ("fine", " 首空白"),
                ("tab", "制\t表"),
                ("good", "好"),
            ]),
        )];
        let resolved = resolve(&[], &plugins);
        assert_eq!(statuses(&resolved), [("p", "good", CommandStatus::Active)]);
    }

    #[test]
    fn the_status_serializes_flat_for_the_settings_page() {
        let resolved = resolve(
            &["uu".to_owned()],
            &[
                ("a".to_owned(), commands(&[("guid", "GUID")])),
                (
                    "b".to_owned(),
                    commands(&[("uuid", "UUID"), ("guidx", "X")]),
                ),
            ],
        );
        assert_eq!(
            serde_json::to_value(&resolved).unwrap(),
            serde_json::json!([
                {"id": "a", "trigger": "guid", "title": "GUID", "status": "active"},
                {"id": "b", "trigger": "uuid", "title": "UUID", "status": "shadowed_by_user_table", "conflict": "uu"},
                {"id": "b", "trigger": "guidx", "title": "X", "status": "shadowed_by_plugin", "conflict_id": "a", "conflict": "guid"},
            ])
        );
    }

    #[test]
    fn random_bytes_fill_the_buffer() {
        let mut first = [0u8; limits::RANDOM_BYTES_PER_EVENT];
        let mut second = [0u8; limits::RANDOM_BYTES_PER_EVENT];
        assert!(random_bytes(&mut first));
        assert!(random_bytes(&mut second));
        // 两次 64 字节全同的概率是 2^-512。
        assert_ne!(first, second);
    }
}
