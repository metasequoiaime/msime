//! The syllable being typed: one optional bopomofo symbol per slot, filled by Dachen keys until a tone key completes it.

use super::layout::{self, Kind};

/// The untoned syllable under construction. Each slot holds at most one symbol; a key of a kind already present replaces it (libchewing's slot rule), so the order the keys were typed in does not matter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PendingSyllable {
    pub initial: Option<char>,
    pub medial: Option<char>,
    pub rime: Option<char>,
}

impl PendingSyllable {
    pub fn is_empty(&self) -> bool {
        self.initial.is_none() && self.medial.is_none() && self.rime.is_none()
    }

    /// Puts `symbol` in the slot of `kind`, replacing whatever was there.
    pub fn insert(&mut self, symbol: char, kind: Kind) {
        *self.slot(kind) = Some(symbol);
    }

    /// Removes the last filled slot in syllable order (rime, then medial, then initial). Returns whether anything was removed.
    pub fn pop(&mut self) -> bool {
        for kind in [Kind::Rime, Kind::Medial, Kind::Initial] {
            if self.slot(kind).take().is_some() {
                return true;
            }
        }
        false
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The symbols in syllable order, e.g. `ㄋㄧ`.
    pub fn bopomofo(&self) -> String {
        self.symbols().collect()
    }

    /// The Dachen keys that spell the filled slots, in syllable order, e.g. `su` for `ㄋㄧ`.
    pub fn keys(&self) -> String {
        let mut keys = String::new();
        self.append_keys(&mut keys);
        keys
    }

    /// Appends the Dachen keys without allocating an intermediate string.
    pub(super) fn append_keys(&self, target: &mut String) {
        for symbol in self.symbols() {
            if let Some((key, _, _)) = layout::DACHEN
                .iter()
                .find(|(_, dachen_symbol, _)| *dachen_symbol == symbol)
            {
                target.push(char::from(*key));
            }
        }
    }

    /// The toned syllable as `zhuyin.db` stores it: the symbols followed by the tone mark (empty for tone 1). `None` while nothing is typed, since a tone alone is not a syllable.
    pub fn toned(&self, mark: &str) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut toned = self.bopomofo();
        toned.push_str(mark);
        Some(toned)
    }

    fn symbols(&self) -> impl Iterator<Item = char> {
        [self.initial, self.medial, self.rime].into_iter().flatten()
    }

    fn slot(&mut self, kind: Kind) -> &mut Option<char> {
        match kind {
            Kind::Initial => &mut self.initial,
            Kind::Medial => &mut self.medial,
            Kind::Rime => &mut self.rime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[u8]) -> PendingSyllable {
        let mut pending = PendingSyllable::default();
        for key in keys {
            let (symbol, kind) = layout::symbol(*key).unwrap();
            pending.insert(symbol, kind);
        }
        pending
    }

    #[test]
    fn slots_fill_in_syllable_order_whatever_the_key_order() {
        assert_eq!(typed(b"su").bopomofo(), "ㄋㄧ");
        assert_eq!(typed(b"us").bopomofo(), "ㄋㄧ");
        assert_eq!(typed(b"lc").bopomofo(), "ㄏㄠ");
        assert_eq!(typed(b"lc").keys(), "cl");
        assert_eq!(typed(b"ju0").keys(), "u0");
    }

    #[test]
    fn a_key_of_a_present_kind_replaces_its_slot() {
        // ㄋ then ㄌ: both initials, so ㄌ replaces ㄋ.
        assert_eq!(typed(b"sx").bopomofo(), "ㄌ");
        // ㄧ then ㄨ: both medials.
        assert_eq!(typed(b"suj").bopomofo(), "ㄋㄨ");
        // ㄠ then ㄚ: both rimes; the initial and medial stay.
        let pending = typed(b"cul8");
        assert_eq!(pending.bopomofo(), "ㄏㄧㄚ");
        assert_eq!(pending.keys(), "cu8");
    }

    #[test]
    fn pop_removes_from_the_end_of_the_syllable() {
        let mut pending = typed(b"l");
        pending.insert('ㄏ', Kind::Initial);
        pending.insert('ㄨ', Kind::Medial);
        let mut shown = vec![pending.bopomofo()];
        while pending.pop() {
            shown.push(pending.bopomofo());
        }
        assert_eq!(shown, ["ㄏㄨㄠ", "ㄏㄨ", "ㄏ", ""]);
        assert!(pending.is_empty());
        assert!(!pending.pop());
    }

    #[test]
    fn toned_appends_the_mark_and_needs_a_symbol() {
        assert_eq!(typed(b"su").toned("ˇ").as_deref(), Some("ㄋㄧˇ"));
        assert_eq!(typed(b"j0").toned("").as_deref(), Some("ㄨㄢ"));
        assert_eq!(typed(b"8").toned("˙").as_deref(), Some("ㄚ˙"));
        assert_eq!(PendingSyllable::default().toned("ˇ"), None);
        let mut pending = typed(b"su");
        pending.clear();
        assert!(pending.is_empty());
    }
}
