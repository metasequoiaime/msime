//! Cantonese input in toneless Jyutping, read against `msime-cantonese.db` (`language_dictionary`). Output is Traditional as stored, and nothing is learned.

pub mod scheme;
pub mod syllable;

use std::path::Path;
use std::sync::Arc;

pub use scheme::{CantoneseCandidate, CantoneseScheme};
pub use syllable::Inventory;

use crate::error::Result;
use crate::language_dictionary::{self, LanguageDictionary};

/// The non-letter keys the scheme spells with while composing: `'` is an explicit syllable boundary.
pub const SPELLING_SYMBOLS_COMPOSING: &str = "'";

/// `msime-cantonese.db` opened for one activation of the scheme, with its syllable inventory read once.
pub struct CantoneseDictionary {
    dictionary: LanguageDictionary,
    inventory: Arc<Inventory>,
}

impl CantoneseDictionary {
    /// Opens `path` read-only; fails as `language_dictionary::open_read_only` does, which the caller reports as the scheme being unavailable.
    pub fn open(path: &Path) -> Result<Self> {
        let dictionary = language_dictionary::open_read_only(path)?;
        let inventory = Arc::new(Inventory::load(&dictionary)?);
        Ok(Self {
            dictionary,
            inventory,
        })
    }

    pub fn dictionary(&self) -> &LanguageDictionary {
        &self.dictionary
    }

    /// The inventory a `CantoneseScheme` segments against.
    pub fn inventory(&self) -> Arc<Inventory> {
        Arc::clone(&self.inventory)
    }
}
