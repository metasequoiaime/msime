//! 离线世界时钟：目录、时区换算、太阳事件、格式与城市持久化。

use crate::{bounded_io, file_lock, storage};
use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike, Utc};
use jiff::tz::{TimeZone as ClockZone, TimeZoneDatabase};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ffi::OsStr, path::Path, sync::OnceLock};
use sunrise::{Coordinates, SolarDay, SolarEvent};
use thiserror::Error;

pub const MAX_CITIES: usize = 32;
const FILE_NAME: &str = "WorldClock.json";
const MAX_FILE_BYTES: u64 = 8192;

#[derive(Clone, Debug, Serialize)]
pub struct City {
    pub id: String,
    pub country: String,
    pub zone: String,
    pub latitude: f64,
    pub longitude: f64,
}

fn coordinate(value: &str, degrees: usize) -> Option<f64> {
    let sign = if value.starts_with('-') { -1.0 } else { 1.0 };
    let digits = value.get(1..)?;
    let d: f64 = digits.get(..degrees)?.parse().ok()?;
    let m: f64 = digits.get(degrees..degrees + 2)?.parse().ok()?;
    let s: f64 = digits
        .get(degrees + 2..)
        .filter(|v| !v.is_empty())
        .unwrap_or("0")
        .parse()
        .ok()?;
    Some(sign * (d + m / 60.0 + s / 3600.0))
}

/// 各国家/地区的代表时区城市；编译内嵌，不访问系统目录或网络。
pub fn catalog() -> &'static [City] {
    static CITIES: OnceLock<Vec<City>> = OnceLock::new();
    CITIES.get_or_init(|| {
        let mut cities = Vec::new();
        for line in include_str!("world_clock/zone.tab")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let columns: Vec<_> = line.split('\t').collect();
            if columns.len() < 3 {
                continue;
            }
            let coordinates = columns[1];
            let Some(split) = coordinates
                .get(1..)
                .and_then(|s| s.find(['+', '-']))
                .map(|n| n + 1)
            else {
                continue;
            };
            let (Some(latitude), Some(longitude)) = (
                coordinate(&coordinates[..split], 2),
                coordinate(&coordinates[split..], 3),
            ) else {
                continue;
            };
            cities.push(City {
                id: columns[2].to_owned(),
                country: columns[0].to_owned(),
                zone: columns[2].to_owned(),
                latitude,
                longitude,
            });
        }
        cities.push(City {
            id: "city:beijing".into(),
            country: "CN".into(),
            zone: "Asia/Shanghai".into(),
            latitude: 39.9042,
            longitude: 116.4074,
        });
        cities
    })
}

fn timezone(name: &str) -> Result<ClockZone, ClockError> {
    static DATABASE: OnceLock<TimeZoneDatabase> = OnceLock::new();
    DATABASE
        .get_or_init(TimeZoneDatabase::bundled)
        .get(name)
        .map_err(|_| ClockError::Invalid)
}

fn zoned(instant: DateTime<Utc>, zone: &ClockZone) -> Result<DateTime<FixedOffset>, ClockError> {
    let timestamp =
        jiff::Timestamp::from_second(instant.timestamp()).map_err(|_| ClockError::Invalid)?;
    let seconds = zone.to_offset(timestamp).seconds();
    let offset = FixedOffset::east_opt(seconds).ok_or(ClockError::Invalid)?;
    Ok(instant.with_timezone(&offset))
}

fn city(id: &str) -> Result<&'static City, ClockError> {
    catalog()
        .iter()
        .find(|c| c.id == id)
        .ok_or(ClockError::Invalid)
}

#[derive(Debug, Error)]
pub enum ClockError {
    #[error("world_clock_invalid")]
    Invalid,
    #[error("world_clock_limit")]
    Limit,
    #[error("world_clock_corrupt")]
    Corrupt,
    #[error("world_clock_io")]
    Io(#[from] std::io::Error),
}

impl ClockError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "world_clock_invalid",
            Self::Limit => "world_clock_limit",
            Self::Corrupt => "world_clock_corrupt",
            Self::Io(_) => "world_clock_io",
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClockCities {
    pub cities: Vec<String>,
}

impl ClockCities {
    fn validate(&self) -> Result<(), ClockError> {
        if self.cities.len() > MAX_CITIES {
            return Err(ClockError::Limit);
        }
        let mut seen = HashSet::new();
        for id in &self.cities {
            city(id)?;
            if !seen.insert(id) {
                return Err(ClockError::Invalid);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClockAction {
    Catalog {},
    Load {},
    Add {
        id: String,
    },
    Remove {
        id: String,
    },
    Snapshot {
        timestamp: i64,
        system_zone: String,
        system_offset: i32,
        cities: Vec<String>,
    },
}

/// 加载和变更在同一把锁下读改写；损坏文件不覆盖。
fn stored(directory: &Path, action: ClockAction) -> Result<ClockCities, ClockError> {
    if !storage::create_directory_and_check(directory)? {
        return Err(ClockError::Invalid);
    }
    let directory = file_lock::open_private_directory(directory)?;
    let lock =
        file_lock::open_private_lock_file_at(&directory, OsStr::new("WorldClock.json.lock"))?;
    let _lock = file_lock::ScopedFileLock::exclusive(lock)?;
    let mut document: ClockCities =
        match file_lock::open_private_file_at(&directory, OsStr::new(FILE_NAME)) {
            Ok(file) => {
                let bytes =
                    bounded_io::read_bounded_file(file, MAX_FILE_BYTES, || ClockError::Corrupt)?;
                serde_json::from_slice(&bytes).map_err(|_| ClockError::Corrupt)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ClockCities::default(),
            Err(error) => return Err(error.into()),
        };
    document.validate().map_err(|_| ClockError::Corrupt)?;
    match action {
        ClockAction::Load {} => return Ok(document),
        ClockAction::Add { id } => {
            city(&id)?;
            if document.cities.contains(&id) {
                return Ok(document);
            }
            document.cities.push(id);
        }
        ClockAction::Remove { id } => {
            city(&id)?;
            document.cities.retain(|c| c != &id);
        }
        _ => return Err(ClockError::Invalid),
    }
    document.validate()?;
    let bytes = serde_json::to_vec(&document).map_err(|_| ClockError::Invalid)?;
    file_lock::write_private_file_at(&directory, OsStr::new(FILE_NAME), &bytes)?;
    Ok(document)
}

#[derive(Debug, Serialize)]
pub struct ClockFormat {
    pub label: &'static str,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ClockCard {
    pub id: String,
    pub zone: String,
    pub time: String,
    pub date: String,
    pub offset: String,
    pub solar_reference: Option<String>,
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
    pub formats: Vec<ClockFormat>,
}

fn card(id: &str, zone: &str, date: DateTime<FixedOffset>, reference: Option<&City>) -> ClockCard {
    let weekday = [
        "星期一",
        "星期二",
        "星期三",
        "星期四",
        "星期五",
        "星期六",
        "星期日",
    ][date.weekday().num_days_from_monday() as usize];
    let full_date = format!(
        "{}年{}月{}日 {}",
        date.year(),
        date.month(),
        date.day(),
        weekday
    );
    let values = [
        ("时间", date.format("%H:%M").to_string()),
        ("时分秒", date.format("%H:%M:%S").to_string()),
        ("日期", date.format("%Y-%m-%d").to_string()),
        ("完整日期", full_date.clone()),
        ("星期", weekday.to_owned()),
        ("日期和时间", date.format("%Y-%m-%d %H:%M:%S").to_string()),
        (
            "中文日期和时间",
            format!("{} {}", full_date, date.format("%H:%M:%S")),
        ),
        (
            "12 小时时间",
            format!(
                "{}{}",
                if date.hour() < 12 { "上午" } else { "下午" },
                date.format("%I:%M:%S")
            ),
        ),
        ("ISO 8601", date.to_rfc3339()),
        ("Unix 时间", date.timestamp().to_string()),
    ];
    let event = |kind| {
        let c = reference?;
        let coordinates = Coordinates::new(c.latitude, c.longitude)?;
        let tz = timezone(&c.zone).ok()?;
        // 日期变更线两侧的经度与法定时区可能相反；只收下转换后落在目标当地日期的事件。
        // 每个事件在自己的瞬间重新取夏令时偏移，不能复用午夜或当前时刻的偏移。
        [-1, 0, 1]
            .into_iter()
            .filter_map(|days| {
                date.date_naive()
                    .checked_add_signed(chrono::Duration::days(days))
            })
            .filter_map(|day| SolarDay::new(coordinates, day).event_time(kind))
            .filter_map(|instant| zoned(instant, &tz).ok())
            .find(|local| local.date_naive() == date.date_naive())
            .map(|local| local.format("%H:%M").to_string())
    };
    ClockCard {
        id: id.into(),
        zone: zone.into(),
        time: date.format("%H:%M:%S").to_string(),
        date: full_date,
        offset: format!("UTC{}", date.format("%:z")),
        solar_reference: reference.map(|c| c.id.clone()),
        sunrise: event(SolarEvent::Sunrise),
        sunset: event(SolarEvent::Sunset),
        formats: values
            .into_iter()
            .map(|(label, text)| ClockFormat { label, text })
            .collect(),
    }
}

/// 快照不读文件，宿主每秒只传已加载的城市列表与当前时钟；非法时钟不降级成错误日期。
pub fn snapshot(
    timestamp: i64,
    system_zone: &str,
    system_offset: i32,
    cities: Vec<String>,
) -> Result<Vec<ClockCard>, ClockError> {
    let cities = ClockCities { cities };
    cities.validate()?;
    let utc = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .filter(|d| (1900..=2100).contains(&d.year()))
        .ok_or(ClockError::Invalid)?;
    let offset = FixedOffset::east_opt(system_offset).ok_or(ClockError::Invalid)?;
    if system_zone.len() > 128 {
        return Err(ClockError::Invalid);
    }
    let reference = catalog().iter().find(|c| c.zone == system_zone);
    let mut cards = vec![card(
        "system",
        system_zone,
        utc.with_timezone(&offset),
        reference,
    )];
    for id in cities.cities {
        let c = city(&id)?;
        let tz = timezone(&c.zone)?;
        let local = zoned(utc, &tz)?;
        cards.push(card(&id, &c.zone, local, Some(c)));
    }
    Ok(cards)
}

pub fn perform(directory: &Path, action: ClockAction) -> Result<serde_json::Value, ClockError> {
    let value = match action {
        ClockAction::Catalog {} => serde_json::to_value(catalog()),
        ClockAction::Snapshot {
            timestamp,
            system_zone,
            system_offset,
            cities,
        } => serde_json::to_value(snapshot(timestamp, &system_zone, system_offset, cities)?),
        action => serde_json::to_value(stored(directory, action)?),
    };
    value.map_err(|_| ClockError::Invalid)
}

#[cfg(test)]
mod tests;
