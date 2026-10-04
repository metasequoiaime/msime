//! Toneless pinyin for emoji, kaomoji and symbol keywords, shaped like pypinyin's `lazy_pinyin`: one item per Han character, and each run of other characters kept as one item.
//!
//! The `pinyin` crate reads each character on its own, while pypinyin picks a polyphone's reading from the phrase around it (重 in 重复 is chong, not zhong). The keywords whose phrase reading differs from the per-character one are listed in `resources/dictionary-sources/pinyin-overrides.txt` (`keyword<TAB>item<TAB>item...`), which keeps the shipped `msime-others.db` keys unchanged. A new polyphone keyword gets the per-character reading until it is added there.

use std::collections::HashMap;

use anyhow::{bail, Result};
use pinyin::ToPinyin;

use crate::text;

pub const OVERRIDES: &str = "pinyin-overrides.txt";

#[derive(Default)]
pub struct Pinyin {
    overrides: HashMap<String, Vec<String>>,
}

impl Pinyin {
    pub fn with_overrides(source: &str) -> Result<Self> {
        let mut overrides = HashMap::new();
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let keyword = fields.next().unwrap_or_default();
            let items: Vec<String> = fields.map(str::to_owned).collect();
            if keyword.is_empty() || items.is_empty() || items.iter().any(String::is_empty) {
                bail!(
                    "{OVERRIDES}:{}: expected keyword<TAB>item[<TAB>item...]",
                    number + 1
                );
            }
            if overrides.insert(keyword.to_owned(), items).is_some() {
                bail!("{OVERRIDES}:{}: {keyword:?} is listed twice", number + 1);
            }
        }
        Ok(Self { overrides })
    }

    pub fn lazy(&self, keyword: &str) -> Vec<String> {
        if let Some(items) = self.overrides.get(keyword) {
            return items.clone();
        }
        per_character(keyword)
    }
}

/// The reading from the `pinyin` crate alone, with ü written as v the way pypinyin's default style does.
pub fn per_character(keyword: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut other = String::new();
    for c in keyword.chars() {
        match c.to_pinyin() {
            Some(reading) => {
                if !other.is_empty() {
                    items.push(std::mem::take(&mut other));
                }
                items.push(reading.plain().replace('ü', "v"));
            }
            None => other.push(c),
        }
    }
    if !other.is_empty() {
        items.push(other);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_follow_lazy_pinyin() {
        assert_eq!(per_character("笑脸"), ["xiao", "lian"]);
        assert_eq!(per_character("剪jj"), ["jian", "jj"]);
        assert_eq!(per_character("T恤衫"), ["T", "xu", "shan"]);
        assert_eq!(per_character("绿"), ["lv"]);
    }

    #[test]
    fn overrides_win_and_reject_malformed_lines() {
        let pinyin = Pinyin::with_overrides("# c\n重复\tchong\tfu\n").unwrap();
        assert_eq!(pinyin.lazy("重复"), ["chong", "fu"]);
        assert_eq!(pinyin.lazy("笑"), ["xiao"]);
        assert!(Pinyin::with_overrides("重复\n").is_err());
        assert!(Pinyin::with_overrides("a\tb\na\tc\n").is_err());
    }

    #[test]
    fn the_repository_overrides_parse() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/dictionary-sources")
            .join(OVERRIDES);
        Pinyin::with_overrides(&std::fs::read_to_string(path).unwrap()).unwrap();
    }
}
