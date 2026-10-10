//! The user journal `msime_user.db` and everything that writes learned state (user-dictionary.md, data-formats.md §6): ranking, fixed positions and pins, removal, pick pairs, typo learning, the personal n-gram store, replay into a generation, generation staging, reset, dictionary-state export and import, and the personal dictionary API. The journal gets the full v4 schema on every new connection and never branches on `user_version`: existing users are at v3.
//!
//! Two owners share this directory: `journal`, `ranking`, `positions`, `removal`, `picks`, `typo_profile` and `ngram_store` (learning) and `replay`, `generation`, `reset`, `state`, `personal` and `bundled` (administration). This file only wires them together.

pub mod bundled;
pub mod generation;
pub mod habits;
pub mod journal;
pub mod ngram_store;
pub mod personal;
pub mod picks;
pub mod positions;
pub mod ranking;
pub mod removal;
pub mod replay;
pub mod reset;
pub mod state;
pub mod typo_profile;
