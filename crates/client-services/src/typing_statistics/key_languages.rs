//! 按键按输入来源折叠到语言；功能键独立归组，历史缺口保留为未分类。

use super::{
    checked_increment, is_known_key_id, validate_day, validate_key_counts, TypingSource,
    TypingStatistics, TypingStatisticsError, TypingStatisticsStore, MAX_COUNT,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 只保存有限键位的计数，不保存按键顺序、时间或输入内容。
pub type KeyCounts = BTreeMap<String, u64>;
/// 宿主在按下时取得输入来源，同一日的多种来源可放在同一批次。
pub type KeySourceCounts = BTreeMap<TypingSource, KeyCounts>;
/// 语言聚合由共享层生成，宿主不用复制方案到语言的映射。
pub type KeyLanguageCounts = BTreeMap<KeyLanguage, KeyCounts>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyLanguage {
    Chinese,
    English,
    Japanese,
    Korean,
    Cantonese,
    Vietnamese,
    Tibetan,
    Common,
    Unknown,
}

impl KeyLanguage {
    fn from_source(source: TypingSource) -> Self {
        match source {
            TypingSource::Quanpin
            | TypingSource::NineKey
            | TypingSource::FourteenKey
            | TypingSource::EighteenKey
            | TypingSource::Shuangpin
            | TypingSource::Ziranma
            | TypingSource::Microsoft
            | TypingSource::Shoudao
            | TypingSource::SmartAbc
            | TypingSource::Sogou
            | TypingSource::Jiajia
            | TypingSource::Ziguang
            | TypingSource::Guobiao
            | TypingSource::Wubi
            | TypingSource::Zhuyin
            | TypingSource::Stroke => Self::Chinese,
            TypingSource::English => Self::English,
            TypingSource::Japanese => Self::Japanese,
            TypingSource::Korean => Self::Korean,
            TypingSource::Cantonese => Self::Cantonese,
            TypingSource::Vietnamese => Self::Vietnamese,
            TypingSource::Tibetan => Self::Tibetan,
            TypingSource::Handwriting
            | TypingSource::Local
            | TypingSource::Ai
            | TypingSource::Reply
            | TypingSource::Voice
            | TypingSource::Unknown => Self::Unknown,
        }
    }
}

/// 通用键只含控制和切换键；可打印的数字、符号和拼字键仍属于按下时的语言。
pub fn is_common_key_id(key: &str) -> bool {
    matches!(
        key,
        "Lang1"
            | "Lang2"
            | "Convert"
            | "NonConvert"
            | "KanaMode"
            | "Space"
            | "Enter"
            | "Backspace"
            | "Tab"
            | "Escape"
            | "Delete"
            | "Insert"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "CapsLock"
            | "ShiftLeft"
            | "ShiftRight"
            | "ControlLeft"
            | "ControlRight"
            | "AltLeft"
            | "AltRight"
            | "MetaLeft"
            | "MetaRight"
            | "Fn"
            | "ContextMenu"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
            | "NumLock"
            | "NumpadEnter"
            | "SoftSymbol"
            | "SoftLayer"
            | "SoftLanguage"
            | "SoftGlobe"
            | "SoftEmoji"
            | "SoftVoice"
    )
}

impl TypingStatistics {
    pub(super) fn validate_key_languages(&self) -> Result<(), TypingStatisticsError> {
        for (day, languages) in &self.daily_key_languages {
            let totals = self
                .daily_keys
                .get(day)
                .ok_or(TypingStatisticsError::InvalidDocument)?;
            let mut summed = KeyCounts::new();
            for (language, keys) in languages {
                validate_key_counts(keys)?;
                for (key, count) in keys {
                    if is_common_key_id(key) != (*language == KeyLanguage::Common) {
                        return Err(TypingStatisticsError::InvalidDocument);
                    }
                    checked_increment(&mut summed, key, *count)?;
                }
            }
            if summed
                .iter()
                .any(|(key, count)| *count > totals.get(key).copied().unwrap_or(0))
            {
                return Err(TypingStatisticsError::InvalidDocument);
            }
        }
        Ok(())
    }

    /// 已校验的旧文件可能只有总数；只补可证明的通用键，其余补为未分类。
    pub(super) fn include_unclassified_keys(&mut self) {
        for (day, keys) in &self.daily_keys {
            let languages = self.daily_key_languages.entry(day.clone()).or_default();
            for (key, total) in keys {
                let classified: u64 = languages
                    .values()
                    .map(|counts| counts.get(key).copied().unwrap_or(0))
                    .sum();
                if classified < *total {
                    let language = if is_common_key_id(key) {
                        KeyLanguage::Common
                    } else {
                        KeyLanguage::Unknown
                    };
                    *languages
                        .entry(language)
                        .or_default()
                        .entry(key.clone())
                        .or_default() += total - classified;
                }
            }
        }
    }
}

impl TypingStatisticsStore {
    /// 兼容没有来源的宿主：总数照常记录，可打印键记为未分类。
    pub fn record_keys(&self, day: &str, keys: &KeyCounts) -> Result<u64, TypingStatisticsError> {
        self.record_keys_by_source(day, keys, None)
    }

    /// 一次加整批每日按键。提供来源时，每个键的分组之和必须恰好等于总数。
    ///
    /// 宿主在按下时分组，在日切换、256 次或 30 秒时写出；不按语言切换额外写盘。
    /// 验证失败、统计关闭或溢出均不写入，保留期与其他统计共用。
    pub fn record_keys_by_source(
        &self,
        day: &str,
        keys: &KeyCounts,
        sources: Option<&KeySourceCounts>,
    ) -> Result<u64, TypingStatisticsError> {
        validate_day(day)?;
        validate_batch(keys)?;
        let mut languages = KeyLanguageCounts::new();
        if let Some(sources) = sources {
            let mut summed = KeyCounts::new();
            for (source, counts) in sources {
                validate_batch(counts)?;
                for (key, count) in counts {
                    checked_increment(&mut summed, key, *count)?;
                    add_language_key(
                        &mut languages,
                        KeyLanguage::from_source(*source),
                        key,
                        *count,
                    )?;
                }
            }
            if &summed != keys {
                return Err(TypingStatisticsError::InvalidKey);
            }
        } else {
            for (key, count) in keys {
                add_language_key(&mut languages, KeyLanguage::Unknown, key, *count)?;
            }
        }
        let presses = keys
            .values()
            .try_fold(0_u64, |sum, count| sum.checked_add(*count))
            .filter(|sum| *sum <= MAX_COUNT)
            .ok_or(TypingStatisticsError::CountExhausted)?;
        if presses == 0 {
            return Ok(0);
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled {
            return Ok(0);
        }
        let day_keys = value.daily_keys.entry(day.to_owned()).or_default();
        for (key, count) in keys {
            checked_increment(day_keys, key, *count)?;
        }
        let day_languages = value.daily_key_languages.entry(day.to_owned()).or_default();
        for (language, keys) in languages {
            for (key, count) in keys {
                checked_increment(day_languages.entry(language).or_default(), &key, count)?;
            }
        }
        if value.last_pruned_day != day {
            value.apply_retention(day);
            value.last_pruned_day = day.to_owned();
        }
        self.write_locked(&lock, &value)?;
        Ok(presses)
    }
}

fn validate_batch(keys: &KeyCounts) -> Result<(), TypingStatisticsError> {
    if keys
        .iter()
        .any(|(key, count)| *count == 0 || !is_known_key_id(key))
    {
        return Err(TypingStatisticsError::InvalidKey);
    }
    Ok(())
}

fn add_language_key(
    languages: &mut KeyLanguageCounts,
    language: KeyLanguage,
    key: &str,
    count: u64,
) -> Result<(), TypingStatisticsError> {
    let language = if is_common_key_id(key) {
        KeyLanguage::Common
    } else {
        language
    };
    checked_increment(languages.entry(language).or_default(), key, count)
}
