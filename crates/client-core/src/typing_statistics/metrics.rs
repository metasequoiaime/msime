//! 统计页的派生指标：从 [`TypingStatistics`] 算出概览、习惯、按键和徽章，所有平台读同一份结果。
//!
//! 口径和 `packages/ui/src/settings/typing-statistics.tsx` 里桌面设置页的 TS 实现逐条一致，`metrics_cases` 测试把两边锁在一起。这里只有纯函数：日期由宿主传入，不读时钟，结果只取决于参数。

use super::{TypingBreakdown, TypingStatistics, HOURS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 一周的天数，概览和按键指标都按「近 7 天」和「再往前 7 天」对比。
const WEEK_DAYS: i64 = 7;
/// 习惯页热力图覆盖的天数：12 周。
const HEATMAP_DAYS: i64 = 84;
/// 首选命中率至少要这么多次选择才给出数值，样本更少时比例没有意义。
const MIN_SELECTION_SAMPLES: u64 = 50;
/// 平均速度只数这几类字符，和 TS 的 `readableCharacters` 相同：数字、标点、表情和符号不是行文，算进来会让输入一串电话号码看起来像打字飞快。
const SPEED_CHARACTER_KINDS: [&str; 3] = ["han", "latin", "otherLetter"];
/// 「快手」要求速度在至少这么多活跃时间上测得，免得几秒钟的输入就解锁。
const SPEED_BADGE_MIN_ACTIVE_MS: u64 = 10 * 60_000;
/// 「神准」要求的选择样本数。
const ACCURACY_BADGE_MIN_SAMPLES: u64 = 500;
/// 「夜猫子」计入的本地小时：23 点到凌晨 3 点。
const NIGHT_HOURS: [usize; 5] = [23, 0, 1, 2, 3];
/// 「早起鸟」计入的本地小时：4 点到 5 点，也就是早上 6 点以前。
const EARLY_HOURS: [usize; 2] = [4, 5];
/// 算作双拼的输入来源。
const SHUANGPIN_SOURCES: [&str; 4] = ["shuangpin", "ziranma", "microsoft", "shoudao"];
/// 每字按键数的分子除了 `KeyA`–`KeyZ`，还有九键上带字母的 2–9 格；其余按键（空格、退格、符号）不是在拼字。
const NINE_SPELLING_KEYS: [&str; 8] = [
    "Nine2", "Nine3", "Nine4", "Nine5", "Nine6", "Nine7", "Nine8", "Nine9",
];

/// 宿主随 `summary` 一起传入、这个模块自己读不到的数值。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SummaryInputs {
    /// 用户自己添加的词条数，宿主用词库的 `count` 操作取得；没有时按 0 计。
    #[serde(default)]
    pub user_words: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DayCount {
    pub day: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OverviewSummary {
    pub week_total: u64,
    pub previous_week_total: u64,
    pub last7: Vec<DayCount>,
    /// 近 7 天每活跃分钟的可读字符数；这 7 天没有任何活跃时间记录时为 `None`。
    pub average_speed: Option<f64>,
    pub previous_average_speed: Option<f64>,
    /// 第一个候选就是所选的比例；样本少于 50 次时为 `None`。
    pub first_candidate_rate: Option<f64>,
    /// 比全拼少按的比例；还没有全拼按键数时为 `None`。
    pub keystrokes_saved_rate: Option<f64>,
    pub current_streak: u64,
    pub longest_streak: u64,
}

/// 一天里计数和最大的连续两小时，`[start, end)`，跨午夜时 `end` 绕回 0 点以后。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PeakWindow {
    pub start: u8,
    pub end: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HabitsSummary {
    pub weeks12: Vec<DayCount>,
    /// 近 7 天逐小时的字数之和。
    pub hours24: Vec<u64>,
    /// 今天以前每天的平均逐小时字数，和 TS 的 `usualHours` 相同；没有可用的天时为 `None`。
    pub usual_hours: Option<Vec<f64>>,
    pub peak_window: Option<PeakWindow>,
    /// 近 12 周里有输入的天数。
    pub active_days: u64,
    /// 全部保留数据的字符类别和来源，未分类的部分计入 `unknown`。
    pub breakdown: TypingBreakdown,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KeysSummary {
    /// 近 7 天每个字用了几次拼字按键；这 7 天没有字或没有任何按键记录时为 `None`。
    pub per_character_keys: Option<f64>,
    pub previous_per_character_keys: Option<f64>,
    /// 近 7 天退格占全部按键的比例；没有按键记录时为 `None`。
    pub backspace_rate: Option<f64>,
    /// 联想上屏占全部上屏的比例；还没有上屏计数时为 `None`。
    pub prediction_rate: Option<f64>,
    pub longest_run: Option<super::TypingRun>,
    /// 第 1、2、3 个候选和其余位置各占的比例；还没有选择记录时为 `None`。
    pub positions: Option<[f64; 4]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AchievementGroup {
    Volume,
    Streak,
    Skill,
    Fun,
}

/// 一枚徽章的固定定义：标题、说明和阈值都是产品文案，写在这里，各平台不各写一份。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AchievementSpec {
    pub id: &'static str,
    /// 奖章上的字，和原型一致。
    pub glyph: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub group: AchievementGroup,
    pub target: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AchievementSummary {
    pub id: &'static str,
    pub glyph: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub group: AchievementGroup,
    /// 解锁那天；还没解锁时为 `None`。
    pub unlocked_day: Option<String>,
    /// 当前进度，和 `target` 同一单位，可能超过 `target`。
    pub current: u64,
    pub target: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TypingSummary {
    pub overview: OverviewSummary,
    pub habits: HabitsSummary,
    pub keys: KeysSummary,
    pub achievements: Vec<AchievementSummary>,
}

/// 16 枚徽章，顺序就是统计页的排列顺序。分组决定奖章的配色，和原型 `proto_script.js` 一致，所以「夜猫子」「早起鸟」和连续天数同组。
const ACHIEVEMENTS: [AchievementSpec; 16] = [
    spec(
        "chars_10k",
        "1万",
        "初出茅庐",
        "累计输入 1 万字",
        AchievementGroup::Volume,
        10_000,
    ),
    spec(
        "chars_100k",
        "10万",
        "十万字",
        "累计输入 10 万字",
        AchievementGroup::Volume,
        100_000,
    ),
    spec(
        "chars_1m",
        "百万",
        "著作等身",
        "累计输入 100 万字",
        AchievementGroup::Volume,
        1_000_000,
    ),
    spec(
        "streak_7",
        "7",
        "连续一周",
        "连续使用 7 天",
        AchievementGroup::Streak,
        7,
    ),
    spec(
        "streak_30",
        "30",
        "连续 30 天",
        "连续使用 30 天",
        AchievementGroup::Streak,
        30,
    ),
    spec(
        "streak_100",
        "100",
        "百日坚持",
        "连续使用 100 天",
        AchievementGroup::Streak,
        100,
    ),
    spec(
        "speed_60",
        "60",
        "快手",
        "平均每分钟 60 字",
        AchievementGroup::Skill,
        60,
    ),
    spec(
        "accuracy_90",
        "准",
        "神准",
        "首选命中超 90%",
        AchievementGroup::Skill,
        90,
    ),
    spec(
        "sentence_1000",
        "整",
        "整句达人",
        "整句上屏 1000 次",
        AchievementGroup::Skill,
        1_000,
    ),
    spec(
        "night_owl",
        "夜",
        "夜猫子",
        "深夜输入 1000 字",
        AchievementGroup::Streak,
        1_000,
    ),
    spec(
        "early_bird",
        "早",
        "早起鸟",
        "早上 6 点前输入",
        AchievementGroup::Streak,
        1,
    ),
    spec(
        "shuangpin_10k",
        "双",
        "双拼玩家",
        "用双拼输入 1 万字",
        AchievementGroup::Skill,
        10_000,
    ),
    spec(
        "handwriting_500",
        "笔",
        "书法家",
        "手写 500 个字",
        AchievementGroup::Fun,
        500,
    ),
    spec(
        "voice_1h",
        "声",
        "动口不动手",
        "语音输入 1 小时",
        AchievementGroup::Fun,
        60,
    ),
    spec(
        "words_50",
        "词",
        "造词者",
        "添加 50 个自定义词",
        AchievementGroup::Fun,
        50,
    ),
    spec(
        "skins_5",
        "肤",
        "换装达人",
        "试用 5 款皮肤",
        AchievementGroup::Fun,
        5,
    ),
];

const fn spec(
    id: &'static str,
    glyph: &'static str,
    title: &'static str,
    description: &'static str,
    group: AchievementGroup,
    target: u64,
) -> AchievementSpec {
    AchievementSpec {
        id,
        glyph,
        title,
        description,
        group,
        target,
    }
}

/// 全部徽章的定义，供宿主在没有统计数据时也能画出锁着的徽章墙。
pub fn achievement_specs() -> &'static [AchievementSpec] {
    &ACHIEVEMENTS
}

/// `id` 是否是一枚已定义的徽章；文件里出现别的 ID 说明文件不是这个版本写的。
pub(super) fn is_achievement_id(id: &str) -> bool {
    ACHIEVEMENTS.iter().any(|spec| spec.id == id)
}

/// 统计页需要的全部派生指标。`today` 是宿主的本地日；它不是合法日期时，按天的窗口都为空。
pub fn summarize(
    statistics: &TypingStatistics,
    today: &str,
    inputs: &SummaryInputs,
) -> TypingSummary {
    let this_week = window(today, 0, WEEK_DAYS);
    let previous_week = window(today, WEEK_DAYS, WEEK_DAYS);
    let heatmap = window(today, 0, HEATMAP_DAYS);
    let count_on = |day: &String| statistics.days.get(day).copied().unwrap_or(0);
    let day_counts = |days: &[String]| {
        days.iter()
            .map(|day| DayCount {
                day: day.clone(),
                count: count_on(day),
            })
            .collect::<Vec<_>>()
    };
    let sum_counts = |days: &[String]| {
        days.iter()
            .fold(0_u64, |sum, day| sum.saturating_add(count_on(day)))
    };

    let recorded: Vec<&str> = statistics.days.keys().map(String::as_str).collect();
    let current_streak = current_streak(&recorded, today);
    let longest_streak = longest_streak(&recorded);
    let average_speed = average_speed_over(statistics, &this_week);
    let selections = statistics.selections.total();
    let first_candidate = statistics.selections.ranks.first().copied().unwrap_or(0);
    let efficiency = &statistics.efficiency;

    let overview = OverviewSummary {
        week_total: sum_counts(&this_week),
        previous_week_total: sum_counts(&previous_week),
        last7: day_counts(&this_week),
        average_speed,
        previous_average_speed: average_speed_over(statistics, &previous_week),
        first_candidate_rate: (selections >= MIN_SELECTION_SAMPLES)
            .then(|| first_candidate as f64 / selections as f64),
        // 输入码比全拼还长时（例如带了辅助码）按 0 计，「少按键」不显示负数。
        keystrokes_saved_rate: (efficiency.spelled_keys > 0).then(|| {
            (1.0 - efficiency.typed_keys as f64 / efficiency.spelled_keys as f64).max(0.0)
        }),
        current_streak,
        longest_streak,
    };

    let hours24 = hours_sum(statistics, &this_week);
    let habits = HabitsSummary {
        weeks12: day_counts(&heatmap),
        peak_window: peak_window(&hours24),
        hours24,
        usual_hours: usual_hours(&statistics.daily_hours, today),
        active_days: heatmap.iter().filter(|day| count_on(day) > 0).count() as u64,
        breakdown: statistics.breakdown(None),
    };

    let keys = KeysSummary {
        per_character_keys: per_character_keys(statistics, &this_week),
        previous_per_character_keys: per_character_keys(statistics, &previous_week),
        backspace_rate: backspace_rate(statistics, &this_week),
        prediction_rate: (efficiency.commits > 0)
            .then(|| efficiency.prediction_commits as f64 / efficiency.commits as f64),
        longest_run: (statistics.longest_run.characters > 0)
            .then(|| statistics.longest_run.clone()),
        positions: positions(statistics),
    };

    let progress = Progress {
        statistics,
        inputs,
        longest_streak,
        week_speed: average_speed,
        week_active_ms: active_ms_over(statistics, &this_week),
    };
    let achievements = ACHIEVEMENTS
        .iter()
        .map(|spec| {
            let (current, met) = progress.of(spec);
            let unlocked_day = statistics
                .achievements
                .get(spec.id)
                .cloned()
                .or_else(|| met.then(|| today.to_owned()));
            AchievementSummary {
                id: spec.id,
                glyph: spec.glyph,
                title: spec.title,
                description: spec.description,
                group: spec.group,
                unlocked_day,
                current,
                target: spec.target,
            }
        })
        .collect();

    TypingSummary {
        overview,
        habits,
        keys,
        achievements,
    }
}

/// 徽章进度需要的、summary 其他部分已经算过的数值。
struct Progress<'a> {
    statistics: &'a TypingStatistics,
    inputs: &'a SummaryInputs,
    longest_streak: u64,
    week_speed: Option<f64>,
    week_active_ms: u64,
}

impl Progress<'_> {
    /// 一枚徽章的当前进度，以及按条件现在是否满足。
    fn of(&self, spec: &AchievementSpec) -> (u64, bool) {
        let statistics = self.statistics;
        let reached = |current: u64| (current, current >= spec.target);
        match spec.id {
            "chars_10k" | "chars_100k" | "chars_1m" => reached(statistics.total),
            "streak_7" | "streak_30" | "streak_100" => reached(self.longest_streak),
            "speed_60" => {
                let speed = self.week_speed.unwrap_or(0.0);
                (
                    speed.floor() as u64,
                    speed >= spec.target as f64 && self.week_active_ms >= SPEED_BADGE_MIN_ACTIVE_MS,
                )
            }
            "accuracy_90" => {
                let total = statistics.selections.total();
                if total < ACCURACY_BADGE_MIN_SAMPLES {
                    return (0, false);
                }
                let rate =
                    statistics.selections.ranks.first().copied().unwrap_or(0) as f64 / total as f64;
                (
                    (rate * 100.0).floor() as u64,
                    rate * 100.0 > spec.target as f64,
                )
            }
            "sentence_1000" => reached(statistics.efficiency.sentence_commits),
            "night_owl" => reached(hour_total(statistics, &NIGHT_HOURS)),
            "early_bird" => reached(hour_total(statistics, &EARLY_HOURS)),
            "shuangpin_10k" => reached(SHUANGPIN_SOURCES.iter().fold(0_u64, |sum, source| {
                sum.saturating_add(statistics.detail.sources.get(*source).copied().unwrap_or(0))
            })),
            "handwriting_500" => reached(
                statistics
                    .detail
                    .sources
                    .get("handwriting")
                    .copied()
                    .unwrap_or(0),
            ),
            "voice_1h" => {
                let milliseconds = statistics
                    .daily_voice_ms
                    .values()
                    .fold(0_u64, |sum, value| sum.saturating_add(*value));
                (milliseconds / 60_000, milliseconds >= spec.target * 60_000)
            }
            "words_50" => reached(self.inputs.user_words.unwrap_or(0)),
            "skins_5" => reached(statistics.skins_tried.len() as u64),
            _ => (0, false),
        }
    }
}

/// 从 `today` 往前数 `offset` 天开始、再往前共 `length` 天，按时间先后排列。
fn window(today: &str, offset: i64, length: i64) -> Vec<String> {
    (0..length)
        .rev()
        .filter_map(|index| crate::calendar::shift_day(today, -(offset + index)))
        .collect()
}

fn readable_characters(statistics: &TypingStatistics, day: &str) -> u64 {
    statistics.daily_details.get(day).map_or(0, |detail| {
        SPEED_CHARACTER_KINDS.iter().fold(0_u64, |sum, kind| {
            sum.saturating_add(detail.characters.get(*kind).copied().unwrap_or(0))
        })
    })
}

fn active_ms_over(statistics: &TypingStatistics, days: &[String]) -> u64 {
    days.iter().fold(0_u64, |sum, day| {
        sum.saturating_add(statistics.daily_active_ms.get(day).copied().unwrap_or(0))
    })
}

/// 每活跃分钟的可读字符数，只计有活跃时间记录的天，和 TS `activityMetrics` 的 `averageSpeed` 同一算法；没有活跃时间时为 `None`。
///
/// `days` 为全部有记录的天时，结果就是 TS 的 `averageSpeed`；概览传入近 7 天。
pub(super) fn average_speed_over(statistics: &TypingStatistics, days: &[String]) -> Option<f64> {
    let mut readable = 0_u64;
    let mut active_ms = 0_u64;
    for day in days {
        let day_active = statistics.daily_active_ms.get(day).copied().unwrap_or(0);
        if day_active > 0 && statistics.days.contains_key(day) {
            active_ms = active_ms.saturating_add(day_active);
            readable = readable.saturating_add(readable_characters(statistics, day));
        }
    }
    (active_ms > 0).then(|| characters_per_minute(readable, active_ms))
}

/// 和 TS 的 `charactersPerMinute` 同样的运算顺序，保证两边得到同一个浮点数。
fn characters_per_minute(characters: u64, active_ms: u64) -> f64 {
    if characters == 0 || active_ms == 0 {
        return 0.0;
    }
    characters as f64 / (active_ms as f64 / 60_000.0)
}

/// 截止到今天的连续天数；今天还没有记录时截止到昨天，因为今天还没过完，不能算断了。
pub(super) fn current_streak(recorded: &[&str], today: &str) -> u64 {
    let present: std::collections::HashSet<&str> = recorded.iter().copied().collect();
    let mut cursor = if present.contains(today) {
        Some(today.to_owned())
    } else {
        crate::calendar::shift_day(today, -1)
    };
    let mut streak = 0_u64;
    while let Some(day) = cursor.filter(|day| present.contains(day.as_str())) {
        streak += 1;
        cursor = crate::calendar::shift_day(&day, -1);
    }
    streak
}

/// 有记录的天里最长的连续天数。`recorded` 按时间先后排列。
pub(super) fn longest_streak(recorded: &[&str]) -> u64 {
    if recorded.is_empty() {
        return 0;
    }
    let mut longest = 1_u64;
    let mut run = 1_u64;
    for pair in recorded.windows(2) {
        if pair[0] == pair[1] {
            continue;
        }
        let next = crate::calendar::shift_day(pair[0], 1);
        run = if next.as_deref() == Some(pair[1]) {
            run + 1
        } else {
            1
        };
        longest = longest.max(run);
    }
    longest
}

/// 今天以前每天的平均逐小时字数，和 TS 的 `usualHours` 相同：只计有完整 24 格且至少有一个字的天。
pub(super) fn usual_hours(
    daily_hours: &BTreeMap<String, Vec<u64>>,
    today: &str,
) -> Option<Vec<f64>> {
    let mut sums = [0_f64; HOURS];
    let mut days = 0_u64;
    for hours in daily_hours
        .iter()
        .filter(|(day, _)| day.as_str() < today)
        .map(|(_, hours)| hours)
    {
        if hours.len() != HOURS || hours.iter().all(|count| *count == 0) {
            continue;
        }
        for (sum, count) in sums.iter_mut().zip(hours) {
            *sum += *count as f64;
        }
        days += 1;
    }
    (days > 0).then(|| sums.iter().map(|sum| sum / days as f64).collect())
}

fn hours_sum(statistics: &TypingStatistics, days: &[String]) -> Vec<u64> {
    let mut sums = vec![0_u64; HOURS];
    for hours in days
        .iter()
        .filter_map(|day| statistics.daily_hours.get(day))
    {
        for (sum, count) in sums.iter_mut().zip(hours) {
            *sum = sum.saturating_add(*count);
        }
    }
    sums
}

/// 计数和最大的连续两小时，并列时取更早的开始时刻；一个字都没有时为 `None`。
fn peak_window(hours: &[u64]) -> Option<PeakWindow> {
    let mut best: Option<(usize, u64)> = None;
    for start in 0..HOURS {
        let sum = hours[start].saturating_add(hours[(start + 1) % HOURS]);
        if sum > 0 && best.is_none_or(|(_, top)| sum > top) {
            best = Some((start, sum));
        }
    }
    best.map(|(start, _)| PeakWindow {
        start: start as u8,
        end: ((start + 2) % HOURS) as u8,
    })
}

/// 全部保留天里这几个小时的字数之和。
fn hour_total(statistics: &TypingStatistics, hours: &[usize]) -> u64 {
    statistics.daily_hours.values().fold(0_u64, |sum, buckets| {
        hours.iter().fold(sum, |sum, hour| {
            sum.saturating_add(buckets.get(*hour).copied().unwrap_or(0))
        })
    })
}

fn key_presses(
    statistics: &TypingStatistics,
    days: &[String],
    wanted: impl Fn(&str) -> bool,
) -> u64 {
    days.iter()
        .filter_map(|day| statistics.daily_keys.get(day))
        .flat_map(|keys| keys.iter())
        .filter(|(key, _)| wanted(key))
        .fold(0_u64, |sum, (_, count)| sum.saturating_add(*count))
}

fn is_spelling_key(key: &str) -> bool {
    key.strip_prefix("Key")
        .is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_uppercase())
        || NINE_SPELLING_KEYS.contains(&key)
}

fn per_character_keys(statistics: &TypingStatistics, days: &[String]) -> Option<f64> {
    let characters = days.iter().fold(0_u64, |sum, day| {
        sum.saturating_add(statistics.days.get(day).copied().unwrap_or(0))
    });
    // 这几天一次按键都没记过，说明宿主不计按键（或者是计按键以前的文件），这时是「不知道」而不是 0。
    let recorded_keys = days
        .iter()
        .any(|day| statistics.daily_keys.contains_key(day));
    (characters > 0 && recorded_keys)
        .then(|| key_presses(statistics, days, is_spelling_key) as f64 / characters as f64)
}

fn backspace_rate(statistics: &TypingStatistics, days: &[String]) -> Option<f64> {
    let total = key_presses(statistics, days, |_| true);
    (total > 0)
        .then(|| key_presses(statistics, days, |key| key == "Backspace") as f64 / total as f64)
}

fn positions(statistics: &TypingStatistics) -> Option<[f64; 4]> {
    let total = statistics.selections.total();
    if total == 0 {
        return None;
    }
    let rank = |index: usize| statistics.selections.ranks.get(index).copied().unwrap_or(0);
    let top = [rank(0), rank(1), rank(2)];
    let rest = total.saturating_sub(top.iter().sum::<u64>());
    let rate = |count: u64| count as f64 / total as f64;
    Some([rate(top[0]), rate(top[1]), rate(top[2]), rate(rest)])
}

#[cfg(test)]
mod tests;
