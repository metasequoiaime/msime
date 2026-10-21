//! 中英日循环切换的状态机：一个键在 中 → 日 → 英 → 中 之间转圈。
//!
//! 旧版 Windows 正式版的 `keybindings.trilingual_cycle`（`installer/default_config/config.default.toml:432`）
//! 打开后，参与循环的是「切换中英文」那一组快捷键，不是 Ctrl+Shift+J；顺序由
//! `engine/contracts/trilingual_input.h` 的 `Next()` 定死：中文 → 日文 → 英文 → 中文。
//! 有组字时先原样上屏（`CommitRaw`：已选中的词段保留，剩下没选的拼音串照原样上屏），
//! 再换模式。英文模式下由同一个切换键回到中文。
//!
//! 这里只有纯函数：宿主读当前模式（`LanguageCycleState`）调 [`next`]，拿到目标模式与
//! 「是否先上屏」，再各自执行。声明了 `HostCapabilities::trilingual_cycle` 的宿主因此共用
//! 同一份顺序和同一份关门行为。
//!
//! 「英文那一态不继续转圈」与「一圈要闭合」不矛盾：Windows 的 `Next()` 在 IME 关着时返回
//! 中文，所以英文里按循环键只回中文，等同于二态切换的回程。宿主从中日英任何一头都能回到
//! 中文，只是英文不会再往日语走。

use crate::preferences::{ChineseScheme, InputScheme};

/// 循环里的三个模式。英文不是方案：它是宿主把键交给应用的那一态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageCycleMode {
    Chinese,
    Japanese,
    English,
}

/// 宿主按下循环键那一刻的状态。`scheme` 是实际在跑的方案（`effective_scheme` 的结果），
/// `last_chinese_scheme` 是要回去的那个中文方案。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCycleState {
    pub mode: LanguageCycleMode,
    pub scheme: InputScheme,
    pub last_chinese_scheme: Option<ChineseScheme>,
    /// 已经选中、钉住的词段还在：这时先上屏，拼音串和已选词一起出去。
    pub has_composition: bool,
}

/// 一次循环的结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCycleStep {
    /// 按完之后要进入的模式。
    pub mode: LanguageCycleMode,
    /// 换模式前先 `CommitRaw` 把正在组的东西上屏。
    pub commit_raw: bool,
    /// 进入中文时要切到的方案。日文固定是日语方案；英文不给方案（键交给应用）。
    pub scheme: Option<InputScheme>,
}

/// 循环键的目标：中 → 日 → 英 → 中。英文那一态只回中文，不再往日语走。
///
/// `enabled` 是 `keybindings.trilingual_cycle`。关着时返回 `None`：这一组快捷键仍是
/// 二态切换，由各宿主原来的路径处理，行为与加这个开关之前完全一样。
pub fn next(enabled: bool, state: LanguageCycleState) -> Option<LanguageCycleStep> {
    if !enabled {
        return None;
    }
    let commit_raw = state.has_composition;
    match state.mode {
        LanguageCycleMode::English => Some(LanguageCycleStep {
            mode: LanguageCycleMode::Chinese,
            commit_raw,
            scheme: Some(chinese_target(state.last_chinese_scheme, state.scheme)),
        }),
        LanguageCycleMode::Chinese => Some(LanguageCycleStep {
            mode: LanguageCycleMode::Japanese,
            commit_raw,
            scheme: Some(InputScheme::Japanese),
        }),
        LanguageCycleMode::Japanese => Some(LanguageCycleStep {
            mode: LanguageCycleMode::English,
            commit_raw,
            scheme: None,
        }),
    }
}

/// 从英文回中文优先用记下的中文方案；没有历史时保留当前中文方案，否则回全拼。
/// 宿主还需按版本与词库可用性选择能运行的中文方案，不能把日语当作中文回退。
pub fn chinese_target(
    last_chinese_scheme: Option<ChineseScheme>,
    scheme: InputScheme,
) -> InputScheme {
    last_chinese_scheme
        .or_else(|| ChineseScheme::of(scheme))
        .map_or(InputScheme::Quanpin, InputScheme::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(mode: LanguageCycleMode) -> LanguageCycleState {
        LanguageCycleState {
            mode,
            scheme: InputScheme::Quanpin,
            last_chinese_scheme: Some(ChineseScheme::Wubi),
            has_composition: false,
        }
    }

    /// 关闭时行为不变：这一组快捷键仍是二态切换，共享层不接管它。
    #[test]
    fn disabled_leaves_the_binary_toggle_alone() {
        for mode in [
            LanguageCycleMode::Chinese,
            LanguageCycleMode::Japanese,
            LanguageCycleMode::English,
        ] {
            assert_eq!(next(false, state(mode)), None);
        }
    }

    /// 打开后走完整一圈：中 → 日 → 英 → 中。顺序与 Windows 的 `Next()` 一致。
    #[test]
    fn enabled_cycles_chinese_japanese_english_and_back_to_chinese() {
        let japanese = next(true, state(LanguageCycleMode::Chinese)).unwrap();
        assert_eq!(japanese.mode, LanguageCycleMode::Japanese);
        // Windows 的 Next() 从中文直接进日语方案，并把当前中文方案记进 last_chinese_scheme。
        assert_eq!(japanese.scheme, Some(InputScheme::Japanese));

        let english = next(true, state(LanguageCycleMode::Japanese)).unwrap();
        assert_eq!(english.mode, LanguageCycleMode::English);
        // 英文不是方案，宿主只换中英模式。
        assert_eq!(english.scheme, None);

        // 英 → 中：Windows 的 Next() 在 ime_enabled 为假时回中文，一圈闭合。
        let chinese = next(true, state(LanguageCycleMode::English)).unwrap();
        assert_eq!(chinese.mode, LanguageCycleMode::Chinese);
        assert_eq!(chinese.scheme, Some(InputScheme::Wubi));
    }

    /// 英文那一态不再往日语走：它只回中文（Windows 的 `Next(false, ·)` 也是中文），
    /// 所以循环键在英文里不会越转越深，也不会绕开「回中文」这一头。
    #[test]
    fn english_mode_only_returns_to_chinese() {
        let step = next(true, state(LanguageCycleMode::English)).unwrap();
        assert_eq!(step.mode, LanguageCycleMode::Chinese);
        assert_ne!(step.scheme, Some(InputScheme::Japanese));
    }

    /// 有组字时先上屏原始字母，再换模式。
    #[test]
    fn composing_commits_raw_before_switching() {
        let mut composing = state(LanguageCycleMode::Chinese);
        composing.has_composition = true;
        let step = next(true, composing).unwrap();
        assert!(step.commit_raw);
        assert_eq!(step.mode, LanguageCycleMode::Japanese);

        let mut japanese_composing = state(LanguageCycleMode::Japanese);
        japanese_composing.has_composition = true;
        assert!(next(true, japanese_composing).unwrap().commit_raw);
    }

    /// 没有组字时不上屏，也不动文档。
    #[test]
    fn idle_does_not_commit_anything() {
        let step = next(true, state(LanguageCycleMode::Chinese)).unwrap();
        assert!(!step.commit_raw);
    }

    /// 从日语回到中文时用的是记下的中文方案，不是当前方案（当前是日语）。
    #[test]
    fn returning_from_japanese_uses_the_last_chinese_scheme() {
        assert_eq!(
            chinese_target(Some(ChineseScheme::Shuangpin), InputScheme::Japanese),
            InputScheme::Shuangpin
        );
        // 日语没有中文历史时回全拼，不能再次进入日语。
        for scheme in [
            InputScheme::Japanese,
            InputScheme::Korean,
            InputScheme::Vietnamese,
            InputScheme::Tibetan,
        ] {
            assert_eq!(chinese_target(None, scheme), InputScheme::Quanpin);
        }
        assert_eq!(
            chinese_target(None, InputScheme::Shuangpin),
            InputScheme::Shuangpin
        );
    }
}
