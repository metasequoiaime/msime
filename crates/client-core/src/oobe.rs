//! 跨平台首次运行引导（OOBE）的状态契约。
//!
//! 各宿主仍负责执行自己的系统设置动作；这里统一“资源、系统输入法、偏好和引导
//! 是否离开过”这四类事实的含义。`None` 表示宿主没有可靠的查询能力，不能被当成
//! `false`，也不能被画成成功。

use serde::{Deserialize, Serialize};

/// 一次 OOBE 运行的事实快照。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct OobeState {
    /// 内置词库和运行时配置是否已经准备好。
    pub resources_ready: Option<bool>,
    /// 输入法是否出现在系统启用列表中。
    pub system_enabled: Option<bool>,
    /// 输入法是否是系统当前输入法。
    pub system_current: Option<bool>,
    /// 首次流程选择的方案和偏好是否已保存。
    pub preferences_saved: Option<bool>,
    /// 用户是否已经离开首次流程；这不是系统输入法状态的替代品。
    pub onboarding_seen: Option<bool>,
}

/// 当前可以直接处理的 OOBE 步骤。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OobeStep {
    PrepareResources,
    EnableSystemInputMethod,
    MakeDefaultInputMethod,
    SavePreferences,
}

impl OobeState {
    /// 资源和偏好已经就绪，且用户已经离开引导。系统输入法状态可能仍未知（例如 iOS）。
    pub fn flow_complete(&self) -> bool {
        self.onboarding_seen == Some(true)
            && self.resources_ready == Some(true)
            && self.preferences_saved == Some(true)
    }

    /// 宿主能够确认输入法可用时的完整条件。
    pub fn system_ready(&self) -> bool {
        self.system_enabled == Some(true) && self.system_current == Some(true)
    }

    /// 所有事实都已确认成功。未知状态不会被当成成功。
    pub fn complete(&self) -> bool {
        self.flow_complete() && self.system_ready()
    }

    /// 返回第一个明确需要用户处理的步骤；未知状态留给宿主显示“无法确认”说明。
    pub fn next_action(&self) -> Option<OobeStep> {
        if self.resources_ready == Some(false) {
            return Some(OobeStep::PrepareResources);
        }
        if self.system_enabled == Some(false) {
            return Some(OobeStep::EnableSystemInputMethod);
        }
        if self.system_current == Some(false) {
            return Some(OobeStep::MakeDefaultInputMethod);
        }
        if self.preferences_saved == Some(false) {
            return Some(OobeStep::SavePreferences);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> OobeState {
        OobeState {
            resources_ready: Some(true),
            system_enabled: Some(true),
            system_current: Some(true),
            preferences_saved: Some(true),
            onboarding_seen: Some(true),
        }
    }

    #[test]
    fn only_confirmed_facts_complete_the_flow() {
        assert!(ready().complete());
        let mut state = ready();
        state.onboarding_seen = None;
        assert!(!state.flow_complete());
        state.onboarding_seen = Some(false);
        assert!(!state.flow_complete());
        state.onboarding_seen = Some(true);
        state.preferences_saved = None;
        assert!(!state.flow_complete());
    }

    #[test]
    fn a_missing_resource_is_the_first_action() {
        let mut state = ready();
        state.resources_ready = Some(false);
        assert_eq!(state.next_action(), Some(OobeStep::PrepareResources));
        assert!(!state.complete());
    }

    #[test]
    fn system_steps_are_ordered_and_independent() {
        let mut state = ready();
        state.system_enabled = Some(false);
        state.system_current = Some(false);
        assert_eq!(state.next_action(), Some(OobeStep::EnableSystemInputMethod));
        state.system_enabled = Some(true);
        assert_eq!(state.next_action(), Some(OobeStep::MakeDefaultInputMethod));
    }

    #[test]
    fn unknown_system_state_is_not_a_failure_or_success() {
        let mut state = ready();
        state.system_enabled = None;
        state.system_current = None;
        assert!(state.flow_complete());
        assert!(!state.system_ready());
        assert!(!state.complete());
        assert_eq!(state.next_action(), None);
    }

    #[test]
    fn seen_does_not_replace_saved_preferences() {
        let mut state = ready();
        state.preferences_saved = Some(false);
        assert!(!state.flow_complete());
        assert_eq!(state.next_action(), Some(OobeStep::SavePreferences));
    }
}
