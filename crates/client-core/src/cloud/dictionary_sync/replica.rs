//! 由日志重放得出的共享状态：每个词条一个「最后写入者胜」的寄存器（删除也是一次写入，留下墓碑），以及写过日志的设备和它们的确认位置。
//!
//! 合并只比较操作自己带的（时间戳、设备编号、是否删除），与操作到达的先后无关：同一组操作按任何顺序、重复任意次合并，结果都一样。墓碑的回收只在封口时做，输入全部来自日志，所有设备在同一个位置做出同样的决定。

use super::clock::Hlc;
use super::entry::WordKey;
use std::collections::BTreeMap;

/// 一台设备超过这么久没有往日志里写任何东西（以日志里最晚的写入时间为准，不看本机时间），封口时就不再等它确认：墓碑照样回收。它回来时从检查点重新加入，见 `docs/dictionary-webdav-sync.md` 的「长期离线的设备」。
pub const LAPSE_SECS: i64 = 90 * 24 * 60 * 60;
/// 设备表的上限。一台设备每重装一次就是一个新编号，表因此会变长；超过时去掉最久没出现的。
pub const MAX_DEVICE_TABLE: usize = 64;
/// 检查点里最多保留的墓碑。超过时按时间戳去掉最早的那些。
pub const MAX_TOMBSTONES: usize = 100_000;

/// 一条操作：给一个词条写一次值。`live` 为假是删除。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    pub key: WordKey,
    pub live: bool,
    pub hlc: Hlc,
}

/// 一个词条现在的值：最后写入它的操作，以及那个操作所在事件的序号（回收墓碑时与确认位置比较）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub live: bool,
    pub hlc: Hlc,
    pub device: String,
    pub seq: u64,
}

impl Record {
    /// 全序：先比时间戳，再比设备编号，最后「存在」胜过「删除」。同一台设备的时间戳严格递增，正常不会走到最后一步；走到了也有确定的结果，而且偏向保留词条。
    fn order(&self) -> (Hlc, &str, bool) {
        (self.hlc, self.device.as_str(), self.live)
    }

    /// `device` 写出的（`hlc`, `live`）是否会取代这个值。
    pub fn loses_to(&self, hlc: Hlc, device: &str, live: bool) -> bool {
        (hlc, device, live) > self.order()
    }
}

/// 一台写过日志的设备。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// 它确认已经写进本机词库的位置：这个序号以内的事件都已经持久地应用了。
    pub ack: u64,
    /// 它最后一次写事件的时间（事件的 `at`，Unix 秒）。
    pub seen: i64,
    /// 被用户移除了。之后它再写事件就取消这个标记。
    pub retired: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replica {
    pub entries: BTreeMap<WordKey, Record>,
    pub devices: BTreeMap<String, DeviceInfo>,
}

impl Replica {
    /// 合并一条操作；返回值是否变了。
    pub fn merge(&mut self, op: &Op, device: &str, seq: u64) -> bool {
        match self.entries.get(&op.key) {
            Some(record) if !record.loses_to(op.hlc, device, op.live) => false,
            _ => {
                self.entries.insert(
                    op.key.clone(),
                    Record {
                        live: op.live,
                        hlc: op.hlc,
                        device: device.to_owned(),
                        seq,
                    },
                );
                true
            }
        }
    }

    /// 记下一台设备写了一个事件：登记或刷新它的条目（同一个编号永远只有一条），推进它的确认位置。`ack` 不能超过这个事件之前的位置。
    pub fn saw(&mut self, device: &str, seq: u64, at: i64, ack: u64) {
        let info = self.devices.entry(device.to_owned()).or_default();
        info.seen = info.seen.max(at);
        info.ack = info.ack.max(ack.min(seq.saturating_sub(1)));
        info.retired = false;
    }

    pub fn retire(&mut self, device: &str) {
        if let Some(info) = self.devices.get_mut(device) {
            info.retired = true;
        }
    }

    pub fn is_live(&self, key: &WordKey) -> bool {
        self.entries.get(key).is_some_and(|record| record.live)
    }

    pub fn live_count(&self) -> usize {
        self.entries.values().filter(|record| record.live).count()
    }

    pub fn tombstone_count(&self) -> usize {
        self.entries.len() - self.live_count()
    }

    /// 封口时回收：参与确认的是没被移除、而且在日志里最晚一次写入之前 [`LAPSE_SECS`] 以内出现过的设备，它们确认位置的最小值以内的墓碑都可以删掉——每台这样的设备都已经把那次删除写进了本机词库。被移除的设备、以及消失了两倍 [`LAPSE_SECS`] 的设备从表里去掉。`seal_seq` 是封口事件的序号，没有参与确认的设备时以它为界。
    pub fn collect(&mut self, seal_seq: u64) {
        let reference = self
            .devices
            .values()
            .map(|info| info.seen)
            .max()
            .unwrap_or(0);
        let horizon = self
            .devices
            .values()
            .filter(|info| !info.retired && info.seen >= reference.saturating_sub(LAPSE_SECS))
            .map(|info| info.ack)
            .min()
            .unwrap_or(seal_seq);
        self.entries
            .retain(|_, record| record.live || record.seq > horizon);
        let tombstones = self.tombstone_count();
        if tombstones > MAX_TOMBSTONES {
            let mut stamps: Vec<(Hlc, WordKey)> = self
                .entries
                .iter()
                .filter(|(_, record)| !record.live)
                .map(|(key, record)| (record.hlc, key.clone()))
                .collect();
            stamps.sort();
            for (_, key) in stamps.into_iter().take(tombstones - MAX_TOMBSTONES) {
                self.entries.remove(&key);
            }
        }
        self.devices.retain(|_, info| {
            !info.retired && info.seen >= reference.saturating_sub(2 * LAPSE_SECS)
        });
        if self.devices.len() > MAX_DEVICE_TABLE {
            let mut seen: Vec<(i64, String)> = self
                .devices
                .iter()
                .map(|(id, info)| (info.seen, id.clone()))
                .collect();
            seen.sort();
            for (_, id) in seen.into_iter().take(self.devices.len() - MAX_DEVICE_TABLE) {
                self.devices.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::entry::WordKind;
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn key(code: &str) -> WordKey {
        WordKey::new(WordKind::Pinyin, code, "水杉").unwrap()
    }

    fn op(code: &str, live: bool, hlc: u64) -> Op {
        Op {
            key: key(code),
            live,
            hlc: Hlc(hlc << 10),
        }
    }

    #[test]
    fn merge_is_order_independent_and_idempotent() {
        let ops = [
            (op("shui'shan", true, 10), A),
            (op("shui'shan", false, 12), B),
            (op("shui'shan", true, 11), A),
            (op("shan", true, 5), B),
        ];
        let mut forward = Replica::default();
        for (seq, (op, device)) in ops.iter().enumerate() {
            forward.merge(op, device, seq as u64 + 1);
        }
        let mut backward = Replica::default();
        for (seq, (op, device)) in ops.iter().enumerate().rev() {
            backward.merge(op, device, seq as u64 + 1);
            backward.merge(op, device, seq as u64 + 1);
        }
        let live = |replica: &Replica| -> Vec<(WordKey, bool, Hlc)> {
            replica
                .entries
                .iter()
                .map(|(key, record)| (key.clone(), record.live, record.hlc))
                .collect()
        };
        assert_eq!(live(&forward), live(&backward));
        assert!(!forward.is_live(&key("shui'shan")));
        assert!(forward.is_live(&key("shan")));
    }

    #[test]
    fn ties_break_by_device_then_in_favour_of_keeping_the_word() {
        let mut replica = Replica::default();
        replica.merge(&op("shui'shan", true, 10), B, 1);
        // 同一时间戳，设备编号大的胜。
        assert!(!replica.merge(&op("shui'shan", false, 10), A, 2));
        assert!(replica.is_live(&key("shui'shan")));
        // 同一台设备、同一时间戳，存在胜过删除。
        assert!(!replica.merge(&op("shui'shan", false, 10), B, 3));
        assert!(replica.is_live(&key("shui'shan")));
    }

    #[test]
    fn a_device_has_one_entry_however_often_it_writes_and_its_ack_never_moves_back() {
        let mut replica = Replica::default();
        for seq in 1..=10 {
            replica.saw(A, seq, 100 + seq as i64, seq.saturating_sub(1));
        }
        replica.saw(A, 11, 50, 2);
        assert_eq!(replica.devices.len(), 1);
        assert_eq!(replica.devices[A].ack, 9);
        assert_eq!(replica.devices[A].seen, 110);
        // 确认位置不能超过这个事件之前。
        replica.saw(B, 3, 100, 50);
        assert_eq!(replica.devices[B].ack, 2);
    }

    #[test]
    fn tombstones_are_collected_only_once_every_active_device_has_acknowledged_them() {
        let mut replica = Replica::default();
        replica.merge(&op("shui'shan", false, 10), A, 5);
        replica.saw(A, 5, 1_000, 4);
        replica.saw(B, 6, 1_000, 4);
        replica.collect(7);
        assert_eq!(replica.tombstone_count(), 1, "B has not applied seq 5 yet");
        replica.saw(B, 8, 1_001, 7);
        replica.saw(A, 9, 1_001, 8);
        replica.collect(10);
        assert_eq!(replica.tombstone_count(), 0);
    }

    #[test]
    fn a_lapsed_or_retired_device_no_longer_holds_back_collection() {
        let mut replica = Replica::default();
        replica.merge(&op("shui'shan", false, 10), A, 5);
        replica.saw(B, 2, 1_000, 1);
        replica.saw(A, 5, 1_000 + LAPSE_SECS + 1, 4);
        replica.saw(A, 6, 1_000 + LAPSE_SECS + 1, 5);
        replica.collect(7);
        assert_eq!(replica.tombstone_count(), 0, "B lapsed");
        assert!(
            replica.devices.contains_key(B),
            "kept until twice the lapse"
        );

        let mut replica = Replica::default();
        replica.merge(&op("shui'shan", false, 10), A, 5);
        replica.saw(B, 2, 1_000, 1);
        replica.saw(A, 6, 1_000, 5);
        replica.retire(B);
        replica.collect(7);
        assert_eq!(replica.tombstone_count(), 0);
        assert!(!replica.devices.contains_key(B));
        // 被移除之后又写了事件（重新批准过）的设备重新参与确认。
        replica.saw(B, 8, 1_001, 1);
        assert!(!replica.devices[B].retired);
    }

    #[test]
    fn the_tombstone_and_device_tables_stay_bounded() {
        let mut replica = Replica::default();
        for index in 0..(MAX_DEVICE_TABLE + 3) {
            replica.saw(&format!("{index:032x}"), 10, 1_000 + index as i64, 0);
        }
        replica.collect(11);
        assert_eq!(replica.devices.len(), MAX_DEVICE_TABLE);
        assert!(!replica.devices.contains_key(&format!("{:032x}", 0)));
    }
}
