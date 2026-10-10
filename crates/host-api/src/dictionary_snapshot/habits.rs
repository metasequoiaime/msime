//! 本地备份里的输入习惯文件（#5659）：整句联想、选词对、拼写纠错、自动纠错抑制和置顶这几张学习表，词库快照格式装不下（见 `msime_engine::user_dictionary::habits`），所以另存成一份 NDJSON，与词库快照的写法一致：
//!
//! - 第一行 `{"type":"header","format":"msime-learning-habits","version":1}`；
//! - 每条输入习惯一行，`type` 是 `bigram`、`trigram`、`pick`、`typo`、`suppression` 或 `pin`，字段见 [`habit_json`]；
//! - 最后一行 `{"type":"footer","records":条数,"sha256":前面所有字节的 SHA-256}`。
//!
//! 读的时候每一行都按字段严格校验（多一个、少一个字段或类型不对都算坏文件），条数和校验和对不上也算坏文件。版本比 1 新的文件不认，旧版本恢复新版本导出的备份时只是少了这一部分。

use super::{
    json, open_snapshot_file, parse_strict_object, reject_symlinked_snapshot_path, BufRead,
    BufReader, Digest, EngineOptions, Path, PendingFiles, Sha256, SnapshotReadError, Value,
};
use msime_engine::host::{habit_is_storable, LearningHabit};

pub(crate) const HABITS_FORMAT: &str = "msime-learning-habits";
const HABITS_VERSION: i64 = 1;
/// 一行最多这么多字节：最长的一行是三元组的三个词，每个至多 4096 字节，JSON 转义后可能再长几倍。
const MAX_HABITS_LINE_BYTES: usize = 256 * 1024;
/// 整个文件最多这么大；整句联想最多 20 万行，远到不了。
const MAX_HABITS_BYTES: u64 = 256 * 1024 * 1024;
/// 与词库状态的记录上限相同。
const MAX_HABIT_RECORDS: usize = 500_000;

/// 待合并的输入习惯在 `preferences_directory` 下的文件名，认领与重试的规则和输入记录相同（[`super::merge_pending_learning`]）。
pub(crate) const HABITS_FILES: PendingFiles = PendingFiles {
    pending: "pending-habits-merge.ndjson",
    claimed: "pending-habits-merge.claimed.ndjson",
    attempts: "pending-habits-merge.attempts",
    abandoned: "habits merge abandoned",
    invalid: &["invalid habits document", "invalid habits file"],
};

/// 一条输入习惯写成的那一行（不含 `\n`）。
fn habit_json(habit: &LearningHabit) -> Value {
    match habit {
        LearningHabit::Bigram {
            previous,
            word,
            count,
        } => json!({"type": "bigram", "previous": previous, "word": word, "count": count}),
        LearningHabit::Trigram {
            earlier,
            previous,
            word,
            count,
        } => json!({
            "type": "trigram", "earlier": earlier, "previous": previous, "word": word, "count": count,
        }),
        LearningHabit::PickTransition {
            previous_key,
            previous_value,
            key,
            value,
            count,
            updated_at,
        } => json!({
            "type": "pick",
            "previous_code": previous_key,
            "previous_word": previous_value,
            "code": key,
            "word": value,
            "count": count,
            "updated_at": updated_at,
        }),
        LearningHabit::TypoCount {
            typed,
            intended,
            accepted,
            updated_at,
        } => json!({
            "type": "typo", "typed": typed, "intended": intended, "accepted": accepted, "updated_at": updated_at,
        }),
        LearningHabit::AutocorrectSuppression {
            input,
            commits,
            updated_at,
        } => {
            json!({"type": "suppression", "input": input, "commits": commits, "updated_at": updated_at})
        }
        LearningHabit::PinnedCandidate {
            context,
            value,
            updated_at,
        } => json!({"type": "pin", "context": context, "word": value, "updated_at": updated_at}),
    }
}

/// [`habit_json`] 的反面：字段必须恰好是那几个、类型正确，并且这条记录能写进日志（`habit_is_storable`）。
fn parse_habit(line: &[u8]) -> Option<LearningHabit> {
    let map = parse_strict_object(line).ok()?;
    let text = |name: &str| map.get(name)?.as_str().map(str::to_owned);
    let number = |name: &str| map.get(name)?.as_i64();
    let keys = |expected: &[&str]| {
        map.len() == expected.len() + 1 && expected.iter().all(|key| map.contains_key(*key))
    };
    let habit = match map.get("type")?.as_str()? {
        "bigram" if keys(&["previous", "word", "count"]) => LearningHabit::Bigram {
            previous: text("previous")?,
            word: text("word")?,
            count: number("count")?,
        },
        "trigram" if keys(&["earlier", "previous", "word", "count"]) => LearningHabit::Trigram {
            earlier: text("earlier")?,
            previous: text("previous")?,
            word: text("word")?,
            count: number("count")?,
        },
        "pick"
            if keys(&[
                "previous_code",
                "previous_word",
                "code",
                "word",
                "count",
                "updated_at",
            ]) =>
        {
            LearningHabit::PickTransition {
                previous_key: text("previous_code")?,
                previous_value: text("previous_word")?,
                key: text("code")?,
                value: text("word")?,
                count: number("count")?,
                updated_at: number("updated_at")?,
            }
        }
        "typo" if keys(&["typed", "intended", "accepted", "updated_at"]) => {
            LearningHabit::TypoCount {
                typed: text("typed")?,
                intended: text("intended")?,
                accepted: number("accepted")?,
                updated_at: number("updated_at")?,
            }
        }
        "suppression" if keys(&["input", "commits", "updated_at"]) => {
            LearningHabit::AutocorrectSuppression {
                input: text("input")?,
                commits: number("commits")?,
                updated_at: number("updated_at")?,
            }
        }
        "pin" if keys(&["context", "word", "updated_at"]) => LearningHabit::PinnedCandidate {
            context: text("context")?,
            value: text("word")?,
            updated_at: number("updated_at")?,
        },
        _ => return None,
    };
    habit_is_storable(&habit).then_some(habit)
}

fn header() -> Value {
    json!({"type": "header", "format": HABITS_FORMAT, "version": HABITS_VERSION})
}

/// 把本机的输入习惯写成 `destination`（绝对路径），写完按读的规则再校验一遍。返回 `{path, habits, skipped}`：`skipped` 是日志里装不下的行（文字为空、超长或含 NUL，计数越界）。日志整体读不出来（有不是 UTF-8 的行等）时报 `habits read rejected`，什么也不写。调用方已经持有词库的会话访问权。
pub(crate) fn export_learning_habits(
    options: &EngineOptions,
    destination: &Path,
) -> Result<Value, &'static str> {
    if !destination.is_absolute() {
        return Err("invalid habits path");
    }
    fn push(body: &mut Vec<u8>, line: &Value) {
        body.extend_from_slice(line.to_string().as_bytes());
        body.push(b'\n');
    }
    let mut body = Vec::new();
    push(&mut body, &header());
    let mut lines = Vec::new();
    let mut skipped = 0usize;
    msime_engine::host::stream_learning_habits(options, &mut |habit| {
        if lines.len() < MAX_HABIT_RECORDS && habit_is_storable(habit) {
            lines.push(habit_json(habit));
        } else {
            skipped += 1;
        }
        true
    })
    .map_err(|_| "habits read rejected")?;
    let habits = lines.len();
    for line in &lines {
        push(&mut body, line);
    }
    let checksum = hex::encode(Sha256::digest(&body));
    push(
        &mut body,
        &json!({"type": "footer", "records": habits, "sha256": checksum}),
    );
    msime_client_core::file_lock::replace_private_file(destination, &body)
        .map_err(|_| "habits file unavailable")?;
    if inspect_learning_habits(destination)? != habits {
        let _ = std::fs::remove_file(destination);
        return Err("invalid habits document");
    }
    Ok(json!({
        "path": destination.to_string_lossy(),
        "habits": habits,
        "skipped": skipped,
    }))
}

/// 逐行读一份输入习惯文件。迭代给出每一条输入习惯；文件读完时核对 footer 的条数和校验和，对不上就在最后给出一个错误，所以消费者（合并的事务）会整体回滚。
struct HabitsReader {
    reader: BufReader<std::fs::File>,
    line: Vec<u8>,
    digest: Sha256,
    records: usize,
    state: ReaderState,
}

#[derive(PartialEq, Eq)]
enum ReaderState {
    Header,
    Body,
    Done,
    Failed,
}

impl HabitsReader {
    fn open(path: &Path) -> Result<Self, &'static str> {
        reject_symlinked_snapshot_path(path)?;
        let file = open_snapshot_file(path).map_err(|_| "habits file unavailable")?;
        let length = file
            .metadata()
            .map_err(|_| "habits file unavailable")?
            .len();
        if length > MAX_HABITS_BYTES {
            return Err("invalid habits file");
        }
        Ok(Self {
            reader: BufReader::new(file),
            line: Vec::new(),
            digest: Sha256::new(),
            records: 0,
            state: ReaderState::Header,
        })
    }

    /// 下一行（不含 `\n`）。读到文件尾返回 `Ok(false)`；最后一行没有 `\n`、行过长、空行或以 `\r` 结尾都算坏文件。
    fn read_line(&mut self) -> Result<bool, &'static str> {
        self.line.clear();
        let read = std::io::Read::take(&mut self.reader, MAX_HABITS_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut self.line)
            .map_err(|_| "habits file unavailable")?;
        if read == 0 {
            return Ok(false);
        }
        if self.line.pop() != Some(b'\n') || self.line.is_empty() || self.line.ends_with(b"\r") {
            return Err("invalid habits document");
        }
        Ok(true)
    }

    /// 读下一条输入习惯；读完并核对过 footer 时返回 `Ok(None)`。
    fn next_habit(&mut self) -> Result<Option<LearningHabit>, &'static str> {
        loop {
            match self.state {
                ReaderState::Done => return Ok(None),
                ReaderState::Failed => return Err("invalid habits document"),
                _ => {}
            }
            if !self.read_line()? {
                return Err("invalid habits document");
            }
            if self.state == ReaderState::Header {
                let map = parse_strict_object(&self.line).map_err(|_| "invalid habits document")?;
                let version = map.get("version").and_then(Value::as_i64);
                if map.len() != 3
                    || map.get("type").and_then(Value::as_str) != Some("header")
                    || map.get("format").and_then(Value::as_str) != Some(HABITS_FORMAT)
                    || version != Some(HABITS_VERSION)
                {
                    return Err("invalid habits document");
                }
                self.digest.update(&self.line);
                self.digest.update(b"\n");
                self.state = ReaderState::Body;
                continue;
            }
            if let Some(habit) = parse_habit(&self.line) {
                self.records += 1;
                if self.records > MAX_HABIT_RECORDS {
                    return Err("invalid habits document");
                }
                self.digest.update(&self.line);
                self.digest.update(b"\n");
                return Ok(Some(habit));
            }
            // 不是一条输入习惯，就只能是 footer，而且它后面不能再有任何东西。
            let map = parse_strict_object(&self.line).map_err(|_| "invalid habits document")?;
            let checksum = hex::encode(std::mem::take(&mut self.digest).finalize());
            let matches = map.len() == 3
                && map.get("type").and_then(Value::as_str) == Some("footer")
                && map.get("records").and_then(Value::as_u64) == Some(self.records as u64)
                && map.get("sha256").and_then(Value::as_str) == Some(checksum.as_str());
            if !matches || self.read_line()? {
                return Err("invalid habits document");
            }
            self.state = ReaderState::Done;
        }
    }
}

impl Iterator for HabitsReader {
    type Item = Result<LearningHabit, SnapshotReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_habit() {
            Ok(Some(habit)) => Some(Ok(habit)),
            Ok(None) => None,
            Err(_) => {
                self.state = ReaderState::Failed;
                // 失败只报一次，之后迭代结束。
                if self.records == usize::MAX {
                    None
                } else {
                    self.records = usize::MAX;
                    Some(Err(SnapshotReadError))
                }
            }
        }
    }
}

/// 完整读一遍 `path`，校验格式、条数和校验和，返回输入习惯的条数。只读。
pub(crate) fn inspect_learning_habits(path: &Path) -> Result<usize, &'static str> {
    let mut reader = HabitsReader::open(path)?;
    while reader.next_habit()?.is_some() {}
    Ok(reader.records)
}

/// 把 `source`（本地备份里的输入习惯文件）校验后原样复制成待合并的文件，键盘收起后空闲时由 [`super::merge_pending_learning`] 合并。还没被认领的一份会被替换。返回 `{queued, habits}`：文件里一条也没有时 `queued` 为假，什么也不写。
pub(crate) fn queue_habits_merge(preferences: &Path, source: &Path) -> Result<Value, &'static str> {
    if !preferences.is_absolute() || !source.is_absolute() {
        return Err("invalid habits path");
    }
    let habits = inspect_learning_habits(source)?;
    if habits == 0 {
        return Ok(json!({"queued": false, "habits": 0}));
    }
    let bytes = std::fs::read(source).map_err(|_| "habits file unavailable")?;
    let pending = preferences.join(HABITS_FILES.pending);
    msime_client_core::file_lock::replace_private_file(&pending, &bytes)
        .map_err(|_| "habits file unavailable")?;
    if inspect_learning_habits(&pending) != Ok(habits) {
        let _ = std::fs::remove_file(&pending);
        return Err("invalid habits document");
    }
    Ok(json!({"queued": true, "habits": habits}))
}

/// 合并认领下来的输入习惯文件：先完整校验一遍（坏文件报 `invalid habits document`，由调用方直接删掉），再在一个事务里合并。
pub(crate) fn merge_claimed_habits(
    options: &EngineOptions,
    claimed: &Path,
) -> Result<Value, &'static str> {
    let habits = inspect_learning_habits(claimed)?;
    let reader = HabitsReader::open(claimed)?;
    let merged = msime_engine::host::merge_learning_habits(options, habits.max(1), reader)
        .map_err(|_| "habits merge rejected")?;
    Ok(json!({
        "merged": true,
        "written": merged.written,
        "kept": merged.kept,
        "trimmed": merged.trimmed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, lines: &[Value]) {
        let mut body = Vec::new();
        for line in lines {
            body.extend_from_slice(line.to_string().as_bytes());
            body.push(b'\n');
        }
        let checksum = hex::encode(Sha256::digest(&body));
        body.extend_from_slice(
            json!({"type": "footer", "records": lines.len() - 1, "sha256": checksum})
                .to_string()
                .as_bytes(),
        );
        body.push(b'\n');
        std::fs::write(path, body).unwrap();
    }

    fn habits() -> Vec<LearningHabit> {
        vec![
            LearningHabit::Bigram {
                previous: "\u{1}".into(),
                word: "我".into(),
                count: 3,
            },
            LearningHabit::Trigram {
                earlier: "我".into(),
                previous: "想".into(),
                word: "去".into(),
                count: 2,
            },
            LearningHabit::PickTransition {
                previous_key: "wo".into(),
                previous_value: "我".into(),
                key: "xiang".into(),
                value: "想".into(),
                count: 9,
                updated_at: 50,
            },
            LearningHabit::TypoCount {
                typed: "jai".into(),
                intended: "jia".into(),
                accepted: 1,
                updated_at: 300,
            },
            LearningHabit::AutocorrectSuppression {
                input: "nihoa".into(),
                commits: 1,
                updated_at: 300,
            },
            LearningHabit::PinnedCandidate {
                context: "hao".into(),
                value: "好".into(),
                updated_at: 300,
            },
        ]
    }

    #[test]
    fn every_kind_round_trips_through_its_line() {
        for habit in habits() {
            let line = habit_json(&habit).to_string();
            assert_eq!(parse_habit(line.as_bytes()), Some(habit));
        }
    }

    #[test]
    fn a_line_with_a_missing_extra_or_mistyped_field_is_rejected() {
        for line in [
            json!({"type": "bigram", "previous": "a", "word": "b"}),
            json!({"type": "bigram", "previous": "a", "word": "b", "count": 1, "extra": 1}),
            json!({"type": "bigram", "previous": "a", "word": "b", "count": "1"}),
            json!({"type": "bigram", "previous": "a", "word": "b", "count": 0}),
            json!({"type": "typo", "typed": "JAI", "intended": "jia", "accepted": 1, "updated_at": 0}),
            json!({"type": "pin", "context": "", "word": "好", "updated_at": 0}),
            json!({"type": "unknown"}),
        ] {
            assert_eq!(parse_habit(line.to_string().as_bytes()), None, "{line}");
        }
    }

    #[test]
    fn a_file_is_read_only_when_the_footer_count_and_checksum_match() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("habits.ndjson");
        let mut lines = vec![header()];
        lines.extend(habits().iter().map(habit_json));
        write(&path, &lines);
        assert_eq!(inspect_learning_habits(&path), Ok(6));
        let read: Vec<_> = HabitsReader::open(&path)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(read, habits());

        // 改动一个字节，校验和对不上。
        let mut bytes = std::fs::read(&path).unwrap();
        let index = bytes.iter().position(|byte| *byte == b'3').unwrap();
        bytes[index] = b'4';
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            inspect_learning_habits(&path),
            Err("invalid habits document")
        );
        let mut reader = HabitsReader::open(&path).unwrap();
        assert!(reader.by_ref().any(|item| item.is_err()));
        assert!(reader.next().is_none());

        // 截掉 footer。
        let text = std::fs::read_to_string(root.path().join("habits.ndjson")).unwrap();
        let cut: String = text
            .lines()
            .take(3)
            .map(|line| format!("{line}\n"))
            .collect();
        std::fs::write(&path, cut).unwrap();
        assert_eq!(
            inspect_learning_habits(&path),
            Err("invalid habits document")
        );

        // 更新的版本不认。
        write(
            &path,
            &[json!({"type": "header", "format": HABITS_FORMAT, "version": 2})],
        );
        assert_eq!(
            inspect_learning_habits(&path),
            Err("invalid habits document")
        );
    }
}
