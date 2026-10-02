//! Toneless Jyutping syllables: the inventory `cantonese.db` lists, and the segmentation of typed letters into it. `'` is a hard boundary no syllable crosses, and only the last letters of the input may be an incomplete syllable, one some inventory syllable starts with.

use std::collections::HashSet;

use crate::error::Result;
use crate::language_dictionary::LanguageDictionary;

/// At most this many segmentations are returned; the first is the one the scheme reads, the rest only matter to callers that want alternatives.
pub const MAX_SEGMENTATIONS: usize = 16;

/// The syllables a dictionary knows, and every proper prefix of them, held in memory for the life of one activation.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    syllables: HashSet<String>,
    prefixes: HashSet<String>,
    longest: usize,
}

impl Inventory {
    /// An inventory of `syllables`, which are lowercase ASCII as `cantonese.db` stores them.
    pub fn new<I, S>(syllables: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut inventory = Self::default();
        for syllable in syllables {
            let syllable = syllable.into();
            for end in 1..syllable.len() {
                inventory.prefixes.insert(syllable[..end].to_owned());
            }
            inventory.longest = inventory.longest.max(syllable.len());
            inventory.syllables.insert(syllable);
        }
        inventory
    }

    /// Reads the `syllables` table once.
    pub fn load(dictionary: &LanguageDictionary) -> Result<Self> {
        Ok(Self::new(dictionary.syllables()?))
    }

    pub fn contains(&self, syllable: &str) -> bool {
        self.syllables.contains(syllable)
    }

    /// Whether some syllable starts with `letters` and is longer than them.
    pub fn is_prefix(&self, letters: &str) -> bool {
        self.prefixes.contains(letters)
    }
}

/// One syllable of a segmentation, as a byte range of the typed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syllable {
    pub start: usize,
    pub end: usize,
    /// False only for a trailing prefix still being typed.
    pub complete: bool,
}

/// A reading of the typed input as syllables, in input order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Segmentation {
    pub syllables: Vec<Syllable>,
}

impl Segmentation {
    /// The syllables' letters out of `input`, the text this segmentation was made from.
    pub fn texts<'a>(&'a self, input: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.syllables
            .iter()
            .map(move |syllable| &input[syllable.start..syllable.end])
    }

    /// The first `count` syllables joined by single spaces: the `entries.key` they are looked up by.
    pub fn key(&self, input: &str, count: usize) -> String {
        let syllables = self.syllables.iter().take(count);
        let capacity = syllables
            .clone()
            .map(|syllable| syllable.end - syllable.start)
            .sum::<usize>()
            .saturating_add(self.syllables.len().min(count).saturating_sub(1));
        let mut key = String::with_capacity(capacity);
        for (index, syllable) in self.syllables.iter().take(count).enumerate() {
            if index > 0 {
                key.push(' ');
            }
            key.push_str(&input[syllable.start..syllable.end]);
        }
        key
    }

    /// Where the letters this segmentation reads end; zero when it reads none.
    pub fn end(&self) -> usize {
        self.syllables.last().map_or(0, |syllable| syllable.end)
    }

    /// Whether the last syllable is an incomplete prefix.
    pub fn ends_in_prefix(&self) -> bool {
        self.syllables
            .last()
            .is_some_and(|syllable| !syllable.complete)
    }
}

/// Every reading of all of `input` (lowercase letters and `'`), by maximal munch with backtracking: at each position the longest piece is tried first, so the first result is the preferred one (`ngoi` before `ngo i`). Empty when no reading covers every letter. A trailing piece that no syllable equals but some syllable starts with counts as an incomplete last syllable.
pub fn segment(input: &str, inventory: &Inventory) -> Vec<Segmentation> {
    segment_with(input, inventory, true)
}

/// `segment`, with `allow_prefix` saying whether the last piece may be an incomplete syllable.
fn segment_with(input: &str, inventory: &Inventory, allow_prefix: bool) -> Vec<Segmentation> {
    let mut found = Vec::with_capacity(MAX_SEGMENTATIONS);
    let mut dead = vec![false; input.len() + 1];
    let mut path = Vec::with_capacity(input.len());
    walk(
        input,
        inventory,
        allow_prefix,
        0,
        &mut path,
        &mut dead,
        &mut found,
    );
    found
}

/// The reading the scheme shows and looks up: the first full segmentation, or when none covers every letter, the preferred reading of the longest leading run of complete syllables, which leaves the letters after it unread.
pub fn best(input: &str, inventory: &Inventory) -> Segmentation {
    if let Some(full) = segment(input, inventory).into_iter().next() {
        return full;
    }
    // Every position a run of complete syllables from the start can end at.
    let mut reachable = vec![false; input.len() + 1];
    reachable[0] = true;
    let mut furthest = 0;
    for position in 0..input.len() {
        if !reachable[position] {
            continue;
        }
        if input.as_bytes()[position] == b'\'' {
            reachable[position + 1] = true;
            continue;
        }
        let chunk_end = chunk_end(input, position);
        for end in position + 1..=chunk_end.min(position + inventory.longest) {
            if inventory.contains(&input[position..end]) {
                reachable[end] = true;
                furthest = furthest.max(end);
            }
        }
    }
    // The run ends in a complete syllable, so its letters are read without a trailing prefix: `hoex` reads `ho e` and leaves `x`, rather than heading for `hoeng` with `hoe`.
    segment_with(&input[..furthest], inventory, false)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The end of the run of letters starting at `position`: the next `'` or the end of the input.
fn chunk_end(input: &str, position: usize) -> usize {
    input[position..]
        .find('\'')
        .map_or(input.len(), |offset| position + offset)
}

/// Extends `path` from `position`; `dead` marks positions already shown to admit no reading of the rest.
fn walk(
    input: &str,
    inventory: &Inventory,
    allow_prefix: bool,
    mut position: usize,
    path: &mut Vec<Syllable>,
    dead: &mut [bool],
    found: &mut Vec<Segmentation>,
) -> bool {
    while input.as_bytes().get(position) == Some(&b'\'') {
        position += 1;
    }
    if position == input.len() {
        found.push(Segmentation {
            syllables: path.clone(),
        });
        return true;
    }
    if dead[position] {
        return false;
    }
    let chunk_end = chunk_end(input, position);
    let mut pieces: Vec<Syllable> = (position + 1..=chunk_end.min(position + inventory.longest))
        .filter(|&end| inventory.contains(&input[position..end]))
        .map(|end| Syllable {
            start: position,
            end,
            complete: true,
        })
        .collect();
    if allow_prefix && chunk_end == input.len() && inventory.is_prefix(&input[position..]) {
        pieces.push(Syllable {
            start: position,
            end: input.len(),
            complete: false,
        });
    }
    // Longest first; a complete syllable sorts before a prefix of the same letters, which the inventory never holds anyway.
    pieces.sort_by_key(|piece| (std::cmp::Reverse(piece.end), !piece.complete));
    let mut any = false;
    for piece in pieces {
        path.push(piece);
        any |= walk(input, inventory, allow_prefix, piece.end, path, dead, found);
        path.pop();
        if found.len() >= MAX_SEGMENTATIONS {
            return true;
        }
    }
    if !any {
        dead[position] = true;
    }
    any
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A slice of the real inventory, enough for the cases below.
    pub(crate) const SYLLABLES: [&str; 22] = [
        "nei", "hou", "ngo", "ngoi", "oi", "gwong", "dung", "waa", "gwo", "ngok", "dei", "di",
        "hoeng", "gong", "jan", "jat", "aa", "m", "ng", "i", "do", "ji",
    ];

    pub(crate) fn inventory() -> Inventory {
        Inventory::new(SYLLABLES)
    }

    fn read(input: &str, segmentation: &Segmentation) -> Vec<String> {
        segmentation
            .syllables
            .iter()
            .map(|syllable| {
                let text = &input[syllable.start..syllable.end];
                if syllable.complete {
                    text.to_owned()
                } else {
                    format!("{text}*")
                }
            })
            .collect()
    }

    fn first(input: &str) -> Vec<String> {
        read(input, &segment(input, &inventory())[0])
    }

    #[test]
    fn segments_the_design_cases() {
        assert_eq!(first("neihou"), ["nei", "hou"]);
        assert_eq!(first("gwongdungwaa"), ["gwong", "dung", "waa"]);
        assert_eq!(first("ngo'oi"), ["ngo", "oi"]);
        assert_eq!(first("ngoi"), ["ngoi"]);
        let all = segment("ngoi", &inventory());
        assert_eq!(all.capacity(), MAX_SEGMENTATIONS);
        let all: Vec<_> = all
            .iter()
            .map(|segmentation| read("ngoi", segmentation))
            .collect();
        assert_eq!(all, [vec!["ngoi"], vec!["ngo", "i"], vec!["ng", "oi"]]);
    }

    #[test]
    fn backtracks_when_the_longest_piece_leads_nowhere() {
        // `abc` is the longest piece, but nothing reads the `d` after it, so the walk backs out to `ab` + `cd`.
        let small = Inventory::new(["ab", "abc", "cd"]);
        let readings = segment("abcd", &small);
        assert_eq!(readings.len(), 1);
        assert_eq!(read("abcd", &readings[0]), ["ab", "cd"]);
        // Greedy `hoeng` leaves `ong`, and no shorter first piece reads either: there is no full reading.
        assert!(segment("hoengong", &inventory()).is_empty());
        assert_eq!(first("hoenggong"), ["hoeng", "gong"]);
    }

    #[test]
    fn the_apostrophe_is_a_hard_boundary() {
        assert!(segment("ngo'i'", &inventory())
            .iter()
            .all(|segmentation| segmentation.syllables.len() == 2));
        assert_eq!(first("ngo'i"), ["ngo", "i"]);
        // Without the apostrophe `ngoi` is one syllable.
        assert_eq!(first("ngoi"), ["ngoi"]);
        // A syllable never spans the boundary, and a prefix may not end before it.
        assert!(segment("ng'oi'x", &inventory()).is_empty());
        assert!(segment("n'ei", &inventory()).is_empty());
        assert_eq!(first("''nei''hou'"), ["nei", "hou"]);
    }

    #[test]
    fn only_the_trailing_letters_may_be_an_incomplete_syllable() {
        assert_eq!(first("neih"), ["nei", "h*"]);
        assert_eq!(first("neiho"), ["nei", "ho*"]);
        // The longer piece wins even when it is only a prefix: `gwon` heads for `gwong`, not `gwo` + `n`.
        assert_eq!(first("gwon"), ["gwon*"]);
        assert_eq!(first("gwongd"), ["gwong", "d*"]);
        assert!(segment("hx", &inventory()).is_empty());
        assert!(segment("", &inventory())
            .iter()
            .all(|s| s.syllables.is_empty()));
    }

    #[test]
    fn best_falls_back_to_the_longest_leading_run() {
        let input = "neihoux";
        let reading = best(input, &inventory());
        assert_eq!(read(input, &reading), ["nei", "hou"]);
        assert_eq!(reading.end(), 6);
        let input = "xnei";
        assert!(best(input, &inventory()).syllables.is_empty());
        let input = "nei'qq";
        assert_eq!(read(input, &best(input, &inventory())), ["nei"]);
    }

    #[test]
    fn the_fallback_reads_only_complete_syllables() {
        // `hoe` is a prefix of `hoeng` and longer than `ho`, but the unread `x` after it means the run must end in complete syllables.
        let small = Inventory::new(["ho", "e", "hoeng", "nei"]);
        let input = "hoex";
        let reading = best(input, &small);
        assert_eq!(read(input, &reading), ["ho", "e"]);
        assert_eq!(reading.end(), 3);
        assert!(!reading.ends_in_prefix());
        let input = "neihoex";
        assert_eq!(read(input, &best(input, &small)), ["nei", "ho", "e"]);
    }

    #[test]
    fn segmentations_are_capped() {
        // `m`, `ng` and `ngoi`-style overlaps multiply; the walk stops at the cap.
        let input = "ng".repeat(12);
        let all = segment(&input, &Inventory::new(["n", "g", "ng"]));
        assert_eq!(all.len(), MAX_SEGMENTATIONS);
        assert_eq!(all[0].syllables.len(), 12);
    }

    #[test]
    fn long_unreadable_input_returns_quickly() {
        let input = format!("{}x", "a".repeat(200));
        assert!(segment(&input, &Inventory::new(["a", "aa"])).is_empty());
        assert_eq!(best(&input, &Inventory::new(["a", "aa"])).end(), 200);
    }

    #[test]
    fn segmentation_keys() {
        let input = "gwong'dungwa";
        let reading = &segment(input, &inventory())[0];
        let key = reading.key(input, 3);
        assert_eq!(key, "gwong dung wa");
        assert_eq!(key.capacity(), key.len());
        assert_eq!(reading.key(input, 2), "gwong dung");
        assert!(reading.ends_in_prefix());
        assert_eq!(reading.end(), input.len());
    }
}
