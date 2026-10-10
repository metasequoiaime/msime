//! The persistent personal n-gram store (user-dictionary.md §10.4): one per journal path for the process. `record` applies to the in-memory model at once and queues the write; a background thread writes about 2 s after the first queued transition or at 64 queued, and decays the tables past `max_transitions`.
//!
//! Lock order is writer, then pending, then model (NSH:487). The model is rebuilt from disk only when `close_cached_journals` ran (the file at the path may be another journal now) or a caller invalidated it; the store's own writes never need a reload because the model already holds them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::time::Duration;

use crate::time::Instant;

use rusqlite::{params, Connection, OpenFlags};

use super::journal::{
    drop_stale_pinyin_upserts, ensure_schema, open_database, open_existing_read_only,
};
use crate::error::{EngineError, Result};
use crate::lattice::ngram::SENTENCE_START;
use crate::lattice::personal::{PersonalNgram, PersonalNgramOptions, PersonalTransition};

pub const FLUSH_DELAY_MS: u64 = 2_000;
pub const FLUSH_BATCH: usize = 64;
/// A journal that cannot be read is not read again sooner than this, so a lasting failure does not cost every keystroke a full read (NS:22).
const RELOAD_RETRY_PAUSE: Duration = Duration::from_secs(10);
/// Reported by `record` when an earlier background write failed; the caller maps it to its own diagnostic.
const WRITE_FAILED: &str = "Personal context could not be written to the journal";

/// 把两张表的计数各减半、丢掉会变成 0 的行（`count>0` 是 CHECK 约束）。衰减（`decay_locked`）和本地备份合并输入习惯后压回上限（`habits::merge_learning_habits`）共用。
pub(crate) const HALVE_SQL: &str = "DELETE FROM personal_bigram WHERE count<2;UPDATE personal_bigram SET count=count/2;DELETE FROM personal_trigram WHERE count<2;UPDATE personal_trigram SET count=count/2;";

/// Bumped by `release_all`: every store's model is stale until it reloads from the file now at its path.
static STORE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// 注册表保留仍被会话或刷新线程使用的 store；下次打开 journal 时会回收只有注册表自身引用的旧路径。
static STORES: LazyLock<Mutex<HashMap<PathBuf, Arc<PersonalNgramStore>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic in another thread while it held the lock leaves plain data behind, which is still what the store wrote last.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn or_start(word: &str) -> &str {
    if word.is_empty() {
        SENTENCE_START
    } else {
        word
    }
}

/// What only the writer touches: the store's own journal connection and the reload back-off.
#[derive(Default)]
struct Writer {
    connection: Option<Connection>,
    retry_after: Option<Instant>,
}

pub struct PersonalNgramStore {
    model: std::sync::RwLock<PersonalNgram>,
    path: PathBuf,
    writer: Mutex<Writer>,
    pending: Mutex<Vec<PersonalTransition>>,
    version: AtomicU64,
    invalidations: AtomicU64,
    loaded_generation: AtomicU64,
    loaded_invalidations: AtomicU64,
    write_failed: AtomicBool,
    probed: AtomicBool,
}

/// `loaded_generation` before the first load, so a new store is stale and reads the file on first use.
const NEVER_LOADED: u64 = u64::MAX;

impl PersonalNgramStore {
    pub fn for_journal(user_db: &Path) -> Arc<PersonalNgramStore> {
        let mut stores = lock(&STORES);
        stores.retain(|_, store| Arc::strong_count(store) > 1);
        Arc::clone(stores.entry(user_db.to_path_buf()).or_insert_with(|| {
            Arc::new(PersonalNgramStore {
                model: RwLock::new(PersonalNgram::new(PersonalNgramOptions::default())),
                path: user_db.to_path_buf(),
                writer: Mutex::new(Writer::default()),
                pending: Mutex::new(Vec::with_capacity(FLUSH_BATCH)),
                version: AtomicU64::new(0),
                invalidations: AtomicU64::new(0),
                loaded_generation: AtomicU64::new(NEVER_LOADED),
                loaded_invalidations: AtomicU64::new(0),
                write_failed: AtomicBool::new(false),
                probed: AtomicBool::new(false),
            })
        }))
    }

    /// The store of a journal if one was ever opened; operations on a journal nobody learned into have nothing queued or held.
    fn existing(user_db: &Path) -> Option<Arc<PersonalNgramStore>> {
        lock(&STORES).get(user_db).cloned()
    }

    fn model_write(&self) -> std::sync::RwLockWriteGuard<'_, PersonalNgram> {
        self.model
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A failure of an earlier background write is reported by the next `record`.
    pub fn record(&self, transitions: &[PersonalTransition]) -> Result<()> {
        if transitions.is_empty() {
            return Ok(());
        }
        self.refresh();
        if !self.probed.load(Ordering::Acquire) {
            // Once per store, so a journal that cannot be written is reported on the commit that first tries.
            let mut writer = lock(&self.writer);
            if !self.probed.load(Ordering::Acquire) {
                self.open_connection(&mut writer)?;
                self.probed.store(true, Ordering::Release);
            }
        }
        let queued = {
            let mut pending = lock(&self.pending);
            {
                let mut model = self.model_write();
                for transition in transitions {
                    model.add(transition);
                }
            }
            pending.extend_from_slice(transitions);
            pending.len()
        };
        self.version.fetch_add(1, Ordering::AcqRel);
        let now = queued >= FLUSH_BATCH;
        schedule_flush(self, now);
        if self.write_failed.load(Ordering::Acquire) {
            return Err(EngineError::failed(WRITE_FAILED));
        }
        Ok(())
    }

    /// The model, reloaded first when the journal was closed and reopened (`close_cached_journals`) or a caller invalidated it.
    pub fn model(&self) -> RwLockReadGuard<'_, PersonalNgram> {
        self.refresh();
        self.model
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Advances whenever the model changes; cached personal-scored answers compare against it.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub fn flush(&self) -> Result<()> {
        let mut writer = lock(&self.writer);
        self.flush_locked(&mut writer)
    }

    /// Take the rows `delete_personal_ngram_word` removed out of the in-memory model without a reload (NS:319-342).
    pub fn forget_removed(&self, removed: &[PersonalTransition]) {
        if removed.is_empty() {
            return;
        }
        let _writer = lock(&self.writer);
        // A stale model is rebuilt from disk on its next access, which already lacks the rows.
        if self.stale() {
            return;
        }
        {
            let mut model = self.model_write();
            for row in removed {
                if row.earlier.is_empty() {
                    model.remove_pair(&row.previous, &row.word, row.times);
                } else {
                    model.remove_triple(&row.earlier, &row.previous, &row.word, row.times);
                }
            }
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    /// Mark this store's model stale without the process-wide generation bump `release_all` makes, so a test can stand in for another connection changing the tables.
    #[cfg(test)]
    pub(crate) fn invalidate_for_tests(&self) {
        self.invalidations.fetch_add(1, Ordering::AcqRel);
    }

    fn stale(&self) -> bool {
        self.loaded_generation.load(Ordering::Acquire) != STORE_GENERATION.load(Ordering::Acquire)
            || self.loaded_invalidations.load(Ordering::Acquire)
                != self.invalidations.load(Ordering::Acquire)
    }

    fn refresh(&self) {
        if !self.stale() {
            return;
        }
        let mut writer = lock(&self.writer);
        self.reload_locked(&mut writer);
    }

    /// NS:420-483. The fresh model is built outside the model lock so a keystroke reading the old counts is never blocked by the disk.
    fn reload_locked(&self, writer: &mut Writer) {
        let generation = STORE_GENERATION.load(Ordering::Acquire);
        let invalidations = self.invalidations.load(Ordering::Acquire);
        let loaded_generation = self.loaded_generation.load(Ordering::Acquire);
        if loaded_generation == generation
            && self.loaded_invalidations.load(Ordering::Acquire) == invalidations
        {
            return;
        }
        // A moved generation means the journal was closed, after which the file at this path may be another journal altogether. Counts loaded from the old one must not outlive a failed reload of the new one.
        let reopened = loaded_generation != generation;
        let now = Instant::now();
        if writer.retry_after.is_some_and(|after| now < after) {
            return;
        }
        let options = *self
            .model
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .options();
        let mut fresh = PersonalNgram::new(options);
        let loaded = match &writer.connection {
            Some(connection) => load_from(connection, &mut fresh),
            // Reading must not create the journal: a session that never learns anything leaves no file behind.
            None => open_existing_read_only(&self.path).and_then(|connection| match connection {
                Some(connection) => load_from(&connection, &mut fresh),
                None => Ok(()),
            }),
        };
        if loaded.is_err() {
            // A busy, damaged or unreadable journal keeps the previous counts and stays stale, so a later access loads again.
            writer.retry_after = Some(now + RELOAD_RETRY_PAUSE);
            let empty = self
                .model
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty();
            if reopened && !empty {
                let pending = lock(&self.pending);
                let mut queued = PersonalNgram::new(options);
                apply_pending(&pending, &mut queued);
                *self.model_write() = queued;
                drop(pending);
                self.version.fetch_add(1, Ordering::AcqRel);
            }
            return;
        }
        writer.retry_after = None;
        {
            let pending = lock(&self.pending);
            apply_pending(&pending, &mut fresh);
            *self.model_write() = fresh;
        }
        self.version.fetch_add(1, Ordering::AcqRel);
        self.loaded_generation.store(generation, Ordering::Release);
        self.loaded_invalidations
            .store(invalidations, Ordering::Release);
    }

    fn open_connection<'a>(&self, writer: &'a mut Writer) -> Result<&'a Connection> {
        if writer.connection.is_none() {
            // The reference ran `ensure_user_database` here, whose connection it opened and closed within the call for any journal but its default one (user_dictionary_journal.cpp:445-452). Going through this thread's journal cache instead would leave the flush thread holding the journal after `close_cached_journals`, so the store's own connection takes the schema pass: it creates the whole journal, not only these two tables.
            let connection = open_database(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )?;
            ensure_schema(&connection)?;
            drop_stale_pinyin_upserts(&connection);
            writer.connection = Some(connection);
        }
        Ok(writer
            .connection
            .as_ref()
            .expect("the connection was opened just above"))
    }

    fn flush_locked(&self, writer: &mut Writer) -> Result<()> {
        let batch = std::mem::take(&mut *lock(&self.pending));
        if batch.is_empty() {
            return Ok(());
        }
        let written = self
            .open_connection(writer)
            .and_then(|connection| write_transitions(connection, &batch));
        if let Err(error) = written {
            // Back in front of anything queued meanwhile, so the next write keeps the order they were recorded in.
            let mut pending = lock(&self.pending);
            let newer = std::mem::replace(&mut *pending, batch);
            pending.extend(newer);
            self.write_failed.store(true, Ordering::Release);
            return Err(error);
        }
        self.write_failed.store(false, Ordering::Release);
        let over_limit = {
            let model = self
                .model
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            model.entries() > model.options().max_transitions
        };
        if over_limit {
            self.decay_locked(writer);
        }
        Ok(())
    }

    /// NS:544-592: halve both tables until the entries fall to three quarters of the limit, so the next commits do not decay again at once. Most counts start at 2 (one explicit pick), and a single halving would remove none of them.
    fn decay_locked(&self, writer: &mut Writer) {
        let Some(connection) = writer.connection.as_ref() else {
            return;
        };
        let options = *self
            .model
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .options();
        let limit = options.max_transitions;
        let low_water = limit - limit / 4;
        let halved = (|| -> Result<()> {
            connection.execute_batch("BEGIN IMMEDIATE")?;
            loop {
                // Rows that would reach 0 are dropped first because count>0 is a CHECK.
                connection.execute_batch(HALVE_SQL)?;
                if count_entries(connection)? <= low_water {
                    break;
                }
            }
            connection.execute_batch("COMMIT")?;
            Ok(())
        })();
        if halved.is_err() {
            // Disk and memory still agree on the undecayed counts; the next record tries again. A failed ROLLBACK means the transaction is already gone.
            if !connection.is_autocommit() {
                let _ = connection.execute_batch("ROLLBACK");
            }
            return;
        }
        let mut fresh = PersonalNgram::new(options);
        if load_from(connection, &mut fresh).is_err() {
            // The halving is committed but could not be read back; reload on next access instead of guessing.
            self.invalidations.fetch_add(1, Ordering::AcqRel);
            return;
        }
        {
            let pending = lock(&self.pending);
            apply_pending(&pending, &mut fresh);
            *self.model_write() = fresh;
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }
}

fn apply_pending(pending: &[PersonalTransition], model: &mut PersonalNgram) {
    for transition in pending {
        model.add(transition);
    }
}

fn clamp_count(count: i64) -> u32 {
    u32::try_from(count.max(0)).unwrap_or(u32::MAX)
}

/// NS:396-418. A journal written before these tables existed simply has no personal data yet.
fn load_from(connection: &Connection, model: &mut PersonalNgram) -> Result<()> {
    let has_tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('personal_bigram','personal_trigram')",
        [],
        |row| row.get(0),
    )?;
    if has_tables < 2 {
        return Ok(());
    }
    if count_entries(connection)? > model.options().max_transitions {
        return Err(EngineError::failed(
            "personal n-gram state exceeds its memory limit",
        ));
    }
    let mut pairs = connection.prepare("SELECT previous,word,count FROM personal_bigram")?;
    let mut rows = pairs.query([])?;
    while let Some(row) = rows.next()? {
        model.add_pair(
            &row.get::<_, String>(0)?,
            &row.get::<_, String>(1)?,
            clamp_count(row.get(2)?),
        );
    }
    let mut triples =
        connection.prepare("SELECT earlier,previous,word,count FROM personal_trigram")?;
    let mut rows = triples.query([])?;
    while let Some(row) = rows.next()? {
        model.add_triple(
            &row.get::<_, String>(0)?,
            &row.get::<_, String>(1)?,
            &row.get::<_, String>(2)?,
            clamp_count(row.get(3)?),
        );
    }
    Ok(())
}

fn count_entries(connection: &Connection) -> Result<usize> {
    let entries: i64 = connection.query_row(
        "SELECT (SELECT count(*) FROM personal_bigram)+(SELECT count(*) FROM personal_trigram)",
        [],
        |row| row.get(0),
    )?;
    Ok(usize::try_from(entries).unwrap_or(0))
}

/// NS:501-542: a bigram per transition, a trigram only when the previous word is not the chain start.
fn write_transitions(connection: &Connection, transitions: &[PersonalTransition]) -> Result<()> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let written = (|| -> Result<()> {
        let mut pair = connection.prepare_cached(
            "INSERT INTO personal_bigram(previous,word,count) VALUES(?1,?2,?3) ON CONFLICT(previous,word) DO UPDATE SET count=count+excluded.count",
        )?;
        let mut triple = connection.prepare_cached(
            "INSERT INTO personal_trigram(earlier,previous,word,count) VALUES(?1,?2,?3,?4) ON CONFLICT(earlier,previous,word) DO UPDATE SET count=count+excluded.count",
        )?;
        for transition in transitions {
            if transition.word.is_empty() || transition.times == 0 {
                continue;
            }
            let previous = or_start(&transition.previous);
            pair.execute(params![previous, transition.word, transition.times])?;
            if previous == SENTENCE_START {
                continue;
            }
            triple.execute(params![
                or_start(&transition.earlier),
                previous,
                transition.word,
                transition.times
            ])?;
        }
        Ok(())
    })()
    .and_then(|()| connection.execute_batch("COMMIT").map_err(EngineError::from));
    if written.is_err() && !connection.is_autocommit() {
        // The batch goes back on the queue; a failed ROLLBACK means the transaction is already gone.
        let _ = connection.execute_batch("ROLLBACK");
    }
    written
}

/// The one thread that writes queued transitions, started by the first store that queues any. It is never joined: whoever needs the rows on disk calls `flush_all`.
struct FlushWorker {
    due: Mutex<HashMap<PathBuf, (Arc<PersonalNgramStore>, Instant)>>,
    wake: Condvar,
}

static FLUSH_WORKER: LazyLock<&'static FlushWorker> = LazyLock::new(|| {
    let worker: &'static FlushWorker = Box::leak(Box::new(FlushWorker {
        due: Mutex::new(HashMap::new()),
        wake: Condvar::new(),
    }));
    std::thread::Builder::new()
        .name("msime-personal-context".to_owned())
        .spawn(move || worker.run())
        .expect("the personal context flush thread could not be started");
    worker
});

impl FlushWorker {
    fn run(&self) {
        let mut due = lock(&self.due);
        loop {
            let Some((path, at)) = due
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(path, (_, at))| (path.clone(), *at))
            else {
                due = self
                    .wake
                    .wait(due)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                continue;
            };
            let now = Instant::now();
            if now < at {
                due = self
                    .wake
                    .wait_timeout(due, at - now)
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .0;
                continue;
            }
            let Some((store, _)) = due.remove(&path) else {
                continue;
            };
            drop(due);
            // A failure stays queued and flagged; the next record reports it and schedules another try.
            let _ = store.flush();
            due = lock(&self.due);
        }
    }
}

fn schedule_flush(store: &PersonalNgramStore, now: bool) {
    let Some(shared) = PersonalNgramStore::existing(&store.path) else {
        return;
    };
    let due = Instant::now()
        + if now {
            Duration::ZERO
        } else {
            Duration::from_millis(FLUSH_DELAY_MS)
        };
    let worker = *FLUSH_WORKER;
    let mut entries = lock(&worker.due);
    entries
        .entry(store.path.clone())
        .and_modify(|(_, at)| *at = (*at).min(due))
        .or_insert((shared, due));
    worker.wake.notify_one();
}

fn all_stores() -> Vec<Arc<PersonalNgramStore>> {
    lock(&STORES).values().cloned().collect()
}

/// Write every store's queue now. The golden harness calls this before reading the journal.
pub fn flush_all() {
    for store in all_stores() {
        // A failed write stays queued and is reported to the next record; nothing here can report it.
        let _ = store.flush();
    }
}

/// Write the queue of the journal's store, if it has one. Every edit of the personal tables through another connection runs this first, or the queued rows would be written back after it.
pub(crate) fn flush_journal(user_db: &Path) -> Result<()> {
    match PersonalNgramStore::existing(user_db) {
        Some(store) => store.flush(),
        None => Ok(()),
    }
}

/// 日志的两张表被别的连接改过（本地备份合并输入习惯）之后调用：这个日志的 store 若还在，把它的模型标成过期，下次用到时从磁盘重读。没有 store 时什么也不做，下次打开时本来就会读盘。
pub(crate) fn invalidate_journal(user_db: &Path) {
    if let Some(store) = PersonalNgramStore::existing(user_db) {
        store.invalidations.fetch_add(1, Ordering::AcqRel);
    }
}

/// `forget_removed` on the journal's store, if it has one.
pub(crate) fn forget_journal_rows(user_db: &Path, removed: &[PersonalTransition]) {
    if let Some(store) = PersonalNgramStore::existing(user_db) {
        store.forget_removed(removed);
    }
}

/// Flush, drop what cannot be written, and close every store's connection.
pub fn release_all() {
    STORE_GENERATION.fetch_add(1, Ordering::AcqRel);
    for store in all_stores() {
        store.close_for_release();
    }
}

impl PersonalNgramStore {
    /// `release_all` for this store alone, without the generation bump that marks every model stale.
    fn close_for_release(&self) {
        let mut writer = lock(&self.writer);
        // The queue belongs to the journal being closed, which the caller may replace or delete next; what cannot be written now is dropped rather than written into whatever file takes its place. Transitions recorded while this runs are kept: they were recorded after the close.
        if self.flush_locked(&mut writer).is_err() {
            lock(&self.pending).clear();
            self.write_failed.store(false, Ordering::Release);
        }
        writer.connection = None;
    }
}

/// Delete every bigram and trigram mentioning `word` through a connection with the journal attached as `schema`, returning what was removed (NS:169-202). Bigram rows come back with an empty `earlier` and trigram rows with theirs, which is never empty on disk (the chain start is stored as `SENTENCE_START`); `forget_removed` takes the list as it is.
pub fn delete_personal_ngram_word(
    connection: &Connection,
    schema: &str,
    word: &str,
) -> Result<Vec<PersonalTransition>> {
    let mut removed = Vec::new();
    {
        let mut pairs = connection.prepare(&format!(
            "SELECT previous,word,count FROM {schema}.personal_bigram WHERE previous=?1 OR word=?1"
        ))?;
        let mut rows = pairs.query(params![word])?;
        while let Some(row) = rows.next()? {
            removed.push(PersonalTransition {
                earlier: String::new(),
                previous: row.get(0)?,
                word: row.get(1)?,
                times: clamp_count(row.get(2)?),
            });
        }
        let mut triples = connection.prepare(&format!(
            "SELECT earlier,previous,word,count FROM {schema}.personal_trigram WHERE earlier=?1 OR previous=?1 OR word=?1"
        ))?;
        let mut rows = triples.query(params![word])?;
        while let Some(row) = rows.next()? {
            removed.push(PersonalTransition {
                earlier: row.get(0)?,
                previous: row.get(1)?,
                word: row.get(2)?,
                times: clamp_count(row.get(3)?),
            });
        }
    }
    connection.execute(
        &format!("DELETE FROM {schema}.personal_bigram WHERE previous=?1 OR word=?1"),
        params![word],
    )?;
    connection.execute(
        &format!(
            "DELETE FROM {schema}.personal_trigram WHERE earlier=?1 OR previous=?1 OR word=?1"
        ),
        params![word],
    )?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::super::journal::test_support::{count, Dir};
    use super::*;

    fn transition(earlier: &str, previous: &str, word: &str, times: u32) -> PersonalTransition {
        PersonalTransition {
            earlier: earlier.to_owned(),
            previous: previous.to_owned(),
            word: word.to_owned(),
            times,
        }
    }

    fn run(words: &[&str], times: u32) -> Vec<PersonalTransition> {
        let mut earlier = "";
        let mut previous = "";
        let mut result = Vec::new();
        for word in words {
            result.push(transition(earlier, previous, word, times));
            earlier = previous;
            previous = word;
        }
        result
    }

    /// test_personal_context_input_session.cpp:356-420: recorded context is held in memory at once, on disk after a flush, and read back by a store that reloads.
    #[test]
    fn recorded_context_reaches_the_journal_and_survives_a_reload() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        assert!(Arc::ptr_eq(
            &store,
            &PersonalNgramStore::for_journal(&journal)
        ));
        assert!(store.model().is_empty());
        assert!(!journal.exists(), "reading the model created the journal");

        let version = store.version();
        store.record(&run(&["我", "想", "去"], 2)).unwrap();
        assert!(store.version() > version);
        assert_eq!(store.model().total(), 6);
        store.flush().unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count FROM personal_bigram WHERE previous=char(1) AND word='我'"
            ),
            2
        );
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_bigram"), 3);
        // The chain start carries no trigram.
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_trigram"), 2);
        assert_eq!(
            count(&journal, "SELECT count(*) FROM personal_trigram WHERE earlier=char(1) AND previous='我' AND word='想'"),
            1
        );

        // Another store reading the same file after a close sees the same counts.
        store.invalidations.fetch_add(1, Ordering::AcqRel);
        let model = store.model();
        assert_eq!(model.total(), 6);
        assert!(model.bigram_probability("我", "想") > 0.8);
    }

    #[test]
    fn pending_context_survives_a_reload() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["甲", "乙"], 1)).unwrap();
        store.flush().unwrap();
        store.record(&run(&["丙"], 2)).unwrap();
        store.invalidations.fetch_add(1, Ordering::AcqRel);
        assert_eq!(
            store.model().total(),
            4,
            "a reload lost the queued transition"
        );
        store.flush().unwrap();
        assert_eq!(count(&journal, "SELECT sum(count) FROM personal_bigram"), 4);
    }

    #[test]
    fn pending_queue_reserves_one_flush_batch() {
        let dir = Dir::new();
        let store = PersonalNgramStore::for_journal(&dir.journal());
        assert!(lock(&store.pending).capacity() >= FLUSH_BATCH);
    }

    #[test]
    fn loading_a_persistent_model_over_the_memory_cap_fails_closed() {
        let dir = Dir::new();
        let journal = dir.journal();
        let connection = Connection::open(&journal).unwrap();
        ensure_schema(&connection).unwrap();
        connection
            .execute_batch(
                "INSERT INTO personal_bigram(previous,word,count) VALUES('a','b',1),('b','c',1);",
            )
            .unwrap();
        let mut model = PersonalNgram::new(PersonalNgramOptions {
            max_transitions: 1,
            ..PersonalNgramOptions::default()
        });

        assert!(load_from(&connection, &mut model).is_err());
    }

    #[test]
    fn a_new_journal_releases_an_unused_store() {
        let directory = Dir::new();
        let first = PersonalNgramStore::for_journal(&directory.journal());
        let released = Arc::downgrade(&first);
        drop(first);
        let other = Dir::new();
        let _next = PersonalNgramStore::for_journal(&other.journal());
        assert!(released.upgrade().is_none(), "空闲仓库仍被注册表永久持有");
    }

    #[test]
    fn a_new_journal_keeps_a_store_used_by_another_session() {
        let directory = Dir::new();
        let first = PersonalNgramStore::for_journal(&directory.journal());
        let other = Dir::new();
        let _next = PersonalNgramStore::for_journal(&other.journal());
        assert!(Arc::ptr_eq(
            &first,
            &PersonalNgramStore::for_journal(&directory.journal()),
        ));
    }

    #[test]
    fn the_background_write_lands_without_a_flush() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        let batch: Vec<PersonalTransition> = (0..FLUSH_BATCH)
            .map(|i| transition("", "", &format!("词{i}"), 1))
            .collect();
        store.record(&batch).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while count(&journal, "SELECT count(*) FROM personal_bigram") < FLUSH_BATCH as i64 {
            assert!(
                Instant::now() < deadline,
                "a full batch was not written at once"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn an_unwritable_journal_is_reported_by_record() {
        let dir = Dir::new();
        let journal = dir.journal();
        std::fs::create_dir(&journal).unwrap();
        let store = PersonalNgramStore::for_journal(&journal);
        assert!(store.record(&run(&["甲"], 2)).is_err());
    }

    /// What `release_all` does to one store's model: the next access must read the file now at the path. Scoped to this store so parallel tests keep theirs.
    fn simulate_close(store: &PersonalNgramStore) {
        lock(&store.writer).connection = None;
        store.loaded_generation.store(
            STORE_GENERATION.load(Ordering::Acquire).wrapping_sub(1),
            Ordering::Release,
        );
    }

    fn with_limit(store: &PersonalNgramStore, max_transitions: usize) {
        *store.model_write() = PersonalNgram::new(PersonalNgramOptions {
            max_transitions,
            ..PersonalNgramOptions::default()
        });
        store
            .loaded_generation
            .store(STORE_GENERATION.load(Ordering::Acquire), Ordering::Release);
    }

    /// test_personal_context_input_session.cpp:376-392: a write the journal rejects stays queued and in memory, is reported by the next record, and lands once the journal accepts writes again.
    #[test]
    fn a_rejected_write_stays_queued_and_is_reported_until_the_journal_accepts_it() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["我", "想"], 2)).unwrap();
        store.flush().unwrap();
        let connection = Connection::open(&journal).unwrap();
        connection
            .execute_batch("CREATE TRIGGER reject_personal BEFORE INSERT ON personal_bigram BEGIN SELECT RAISE(FAIL,'injected'); END;")
            .unwrap();
        store.record(&[transition("我", "想", "去", 2)]).unwrap();
        assert!(store.flush().is_err());
        assert!(
            store.model().bigram_probability("想", "去") > 0.0,
            "a rejected write left memory"
        );
        let reported = store
            .record(&[transition("我", "想", "去", 2)])
            .unwrap_err();
        assert!(reported.to_string().contains(WRITE_FAILED), "{reported}");
        connection
            .execute_batch("DROP TRIGGER reject_personal;")
            .unwrap();
        store.flush().unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count FROM personal_bigram WHERE previous='想' AND word='去'"
            ),
            4
        );
        store.record(&[transition("", "", "好", 1)]).unwrap();
    }

    /// test_personal_context_input_session.cpp:393-397: a record under one batch is not written at once but scheduled on the background thread about `FLUSH_DELAY_MS` later, and that write lands without anyone flushing. Other tests call `flush_all` at any moment, which may write this store early, so the delay is read from the worker's schedule rather than from when the row appears.
    #[test]
    fn a_small_batch_is_written_later_not_synchronously() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        let start = Instant::now();
        store.record(&[transition("", "", "迟", 1)]).unwrap();
        let due = lock(&FLUSH_WORKER.due).get(&journal).map(|(_, at)| *at);
        let due = due.expect("a small batch was not scheduled");
        assert!(
            due >= start + Duration::from_millis(FLUSH_DELAY_MS),
            "a small batch was scheduled {:?} after the record",
            due.saturating_duration_since(start)
        );
        let deadline = start + Duration::from_secs(10);
        while count(&journal, "SELECT count(*) FROM personal_bigram") < 1 {
            assert!(Instant::now() < deadline, "the delayed write never landed");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// test_personal_context_input_session.cpp:398-413: once the journal was closed, a file at its path that cannot be read must not leave the previous journal's counts in service, and the failed read is not retried on every access.
    #[test]
    fn an_unreadable_journal_after_a_close_stops_serving_the_old_counts() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["甲"], 2)).unwrap();
        store.flush().unwrap();
        assert!(!store.model().is_empty());
        simulate_close(&store);
        std::fs::remove_file(&journal).unwrap();
        std::fs::write(&journal, vec![b'x'; 4096]).unwrap();
        assert!(
            store.model().is_empty(),
            "a journal that could not be read after a close kept serving the previous counts"
        );
        assert!(store.stale());
        assert!(lock(&store.writer).retry_after.is_some());
        assert!(store.model().is_empty());
    }

    /// `release_all`: a queue the closing journal refuses is dropped, never written into whatever file takes its place.
    #[test]
    fn a_queue_that_cannot_be_written_at_close_is_dropped() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["甲"], 2)).unwrap();
        store.flush().unwrap();
        let connection = Connection::open(&journal).unwrap();
        connection
            .execute_batch("CREATE TRIGGER reject_personal BEFORE INSERT ON personal_bigram BEGIN SELECT RAISE(FAIL,'injected'); END;")
            .unwrap();
        store.record(&[transition("", "甲", "丢", 2)]).unwrap();
        store.close_for_release();
        assert!(lock(&store.pending).is_empty());
        assert!(!store.write_failed.load(Ordering::Acquire));
        assert!(lock(&store.writer).connection.is_none());
        connection
            .execute_batch("DROP TRIGGER reject_personal;")
            .unwrap();
        store.record(&[transition("", "甲", "留", 2)]).unwrap();
        store.flush().unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM personal_bigram WHERE word='丢'"
            ),
            0
        );
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM personal_bigram WHERE word='留'"
            ),
            1
        );
    }

    /// user_dictionary_journal.cpp:445-452: the reference opened msime's journal per call, so the thread that writes the queue keeps no journal handle of its own afterwards.
    #[test]
    fn the_thread_that_writes_the_queue_keeps_no_journal_handle() {
        use super::super::journal::{release_thread_journal, thread_holds_journal};
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["甲"], 2)).unwrap();
        release_thread_journal();
        simulate_close(&store);
        store.record(&[transition("", "甲", "乙", 2)]).unwrap();
        let writer = Arc::clone(&store);
        let held = std::thread::spawn(move || {
            writer.flush().unwrap();
            thread_holds_journal()
        })
        .join()
        .unwrap();
        assert!(!held, "the writing thread cached the journal connection");
        assert!(!thread_holds_journal());
        assert_eq!(
            count(
                &journal,
                "SELECT count FROM personal_bigram WHERE previous='甲' AND word='乙'"
            ),
            2
        );
    }

    /// test_personal_context_input_session.cpp:415-437: pair and triple counts halve together, and the model read back after the decay matches what a fresh read of the journal gives.
    #[test]
    fn decay_halves_pairs_and_triples_and_memory_matches_disk() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        with_limit(&store, 3);
        store
            .record(&[transition("", "", "甲", 4), transition("", "甲", "乙", 3)])
            .unwrap();
        assert_eq!(store.model().entries(), 3);
        store.record(&[transition("甲", "乙", "丙", 1)]).unwrap();
        store.flush().unwrap();
        let pair = |previous: &str, word: &str| {
            count(&journal, &format!("SELECT coalesce(sum(count),0) FROM personal_bigram WHERE previous={previous} AND word='{word}'"))
        };
        assert_eq!(pair("char(1)", "甲"), 2);
        assert_eq!(pair("'甲'", "乙"), 1);
        assert_eq!(pair("'乙'", "丙"), 0);
        assert_eq!(
            count(&journal, "SELECT coalesce(sum(count),0) FROM personal_trigram WHERE earlier=char(1) AND previous='甲' AND word='乙'"),
            1
        );
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM personal_trigram WHERE word='丙'"
            ),
            0
        );
        let (entries, probability) = {
            let model = store.model();
            (
                model.entries(),
                model.probability(&model.context(None, Some("甲")), "乙"),
            )
        };
        assert_eq!(entries, 3);
        let mut fresh = PersonalNgram::new(PersonalNgramOptions {
            max_transitions: 3,
            ..PersonalNgramOptions::default()
        });
        load_from(&Connection::open(&journal).unwrap(), &mut fresh).unwrap();
        assert_eq!(fresh.entries(), entries);
        assert_eq!(
            fresh.probability(&fresh.context(None, Some("甲")), "乙"),
            probability
        );
    }

    /// test_personal_context_input_session.cpp:439-452: when every count is 2 one halving removes nothing, so the decay repeats until the entries are at most three quarters of the limit.
    #[test]
    fn decay_repeats_until_three_quarters_when_every_count_is_two() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        with_limit(&store, 4);
        store
            .record(&[transition("", "", "甲", 2), transition("", "甲", "乙", 2)])
            .unwrap();
        store.record(&[transition("甲", "乙", "丙", 2)]).unwrap();
        store.flush().unwrap();
        let rows = count(
            &journal,
            "SELECT (SELECT count(*) FROM personal_bigram)+(SELECT count(*) FROM personal_trigram)",
        );
        assert!(rows <= 3, "{rows} rows left after the decay");
        let entries = store.model().entries();
        assert!(entries <= 3, "{entries} entries left in memory");
        assert_eq!(entries as i64, rows);
        let mut fresh = PersonalNgram::new(PersonalNgramOptions::default());
        load_from(&Connection::open(&journal).unwrap(), &mut fresh).unwrap();
        assert_eq!(fresh.entries(), entries);
    }

    /// test_personal_context_input_session.cpp:415-452: past the limit the tables halve until three quarters of it remain, and the model follows.
    #[test]
    fn decay_halves_past_the_limit() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        *store.model_write() = PersonalNgram::new(PersonalNgramOptions {
            max_transitions: 8,
            ..PersonalNgramOptions::default()
        });
        store
            .loaded_generation
            .store(STORE_GENERATION.load(Ordering::Acquire), Ordering::Release);
        let mut batch: Vec<PersonalTransition> = (0..6)
            .map(|i| transition("", "", &format!("常{i}"), 4))
            .collect();
        batch.extend((0..6).map(|i| transition("", "", &format!("稀{i}"), 1)));
        store.record(&batch).unwrap();
        store.flush().unwrap();
        // The rare rows go, the frequent ones halve: 6 entries is at most 8 - 8/4.
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_bigram"), 6);
        assert_eq!(
            count(&journal, "SELECT sum(count) FROM personal_bigram"),
            12
        );
        assert_eq!(store.model().entries(), 6);
        assert_eq!(store.model().total(), 12);
    }

    #[test]
    fn deleting_a_word_removes_every_mention_on_disk_and_in_memory() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        store.record(&run(&["我", "想", "去"], 2)).unwrap();
        store.record(&run(&["他", "去"], 1)).unwrap();
        store.flush().unwrap();
        let connection = Connection::open(&journal).unwrap();
        let removed = delete_personal_ngram_word(&connection, "main", "想").unwrap();
        // (我,想) (想,去) and the trigrams (START,我,想) (我,想,去).
        assert_eq!(removed.len(), 4);
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM personal_bigram WHERE previous='想' OR word='想'"
            ),
            0
        );
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_trigram"), 1);
        store.forget_removed(&removed);
        let model = store.model();
        assert_eq!(model.bigram_probability("我", "想"), 0.0);
        assert_eq!(model.confidence("想"), 0.0);
        assert!(model.confidence("他") > 0.0);
    }

    /// test_personal_context_input_session.cpp:510-543: concurrent reads, writes and reloads neither produce an invalid probability nor lose or double-count a transition. Reloads bump this store's own `invalidations`; `release_all` would close the stores of tests running in parallel.
    #[test]
    fn concurrent_reads_writes_and_reloads_keep_every_transition() {
        let dir = Dir::new();
        let journal = dir.journal();
        let store = PersonalNgramStore::for_journal(&journal);
        std::thread::scope(|scope| {
            for _ in 0..3 {
                scope.spawn(|| {
                    for _ in 0..300 {
                        let model = store.model();
                        let probability = model.probability(&model.context(None, Some("甲")), "乙");
                        assert!(
                            (0.0..=1.0).contains(&probability),
                            "a concurrent read saw {probability}"
                        );
                    }
                });
            }
            scope.spawn(|| {
                for _ in 0..100 {
                    store.record(&[transition("", "甲", "乙", 1)]).unwrap();
                }
            });
            for _ in 0..50 {
                store.invalidations.fetch_add(1, Ordering::AcqRel);
                store.flush().unwrap();
            }
        });
        store.flush().unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count FROM personal_bigram WHERE previous='甲' AND word='乙'"
            ),
            100
        );
        store.invalidations.fetch_add(1, Ordering::AcqRel);
        assert!(store.model().bigram_probability("甲", "乙") > 0.0);
    }
}
