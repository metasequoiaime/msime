//! 网格辅筛操作只改变本会话，普通拼音入口仍保持拼音含义。

use super::*;
use crate::grid_helpcode::{
    group_letters, GridHelpcodeAction as Action, GridHelpcodeAvailability as Availability,
    GridHelpcodeSlot, GridHelpcodeSnapshot,
};

impl NineKeySession {
    pub(crate) fn configure_grid_helpcode(
        &mut self,
        enabled: bool,
        table: Option<crate::helpcode::SharedKeymap>,
    ) {
        if self.grid_helpcode_enabled == enabled
            && match (&self.grid_helpcode_table, &table) {
                (Some(old), Some(new)) => std::sync::Arc::ptr_eq(old, new),
                (None, None) => true,
                _ => false,
            }
        {
            return;
        }
        let active = self.grid_helpcode.active();
        self.grid_helpcode.clear();
        self.grid_helpcode_enabled = enabled;
        self.grid_helpcode_table = table;
        self.grid_helpcode.changed();
        if active {
            self.refresh();
        }
    }

    pub(crate) fn replace_grid_helpcode_table(
        &mut self,
        table: Option<crate::helpcode::SharedKeymap>,
    ) {
        self.configure_grid_helpcode(self.grid_helpcode_enabled, table);
    }

    pub(crate) fn grid_helpcode_snapshot(&self) -> GridHelpcodeSnapshot {
        let availability = if !self.grid_helpcode_enabled {
            Availability::Disabled
        } else if !self.active() {
            Availability::NotComposing
        } else if self.english_only || !self.pinyin {
            Availability::Unsupported
        } else if self
            .grid_helpcode_table
            .as_ref()
            .is_none_or(|map| map.is_empty())
        {
            Availability::MissingTable
        } else {
            Availability::Available
        };
        self.grid_helpcode.snapshot(availability)
    }

    pub(crate) fn grid_helpcode_action(&mut self, generation: u64, action: Action) -> KeyResult {
        if generation != self.grid_helpcode.generation
            || self.grid_helpcode_snapshot().availability != Availability::Available
        {
            return KeyResult::unhandled();
        }
        let mut refresh = false;
        match action {
            Action::Begin if !self.grid_helpcode.editing => {
                self.grid_helpcode.editing = true;
                self.grid_helpcode.uppercase = false;
                refresh = !self.strokes.is_empty();
                self.strokes.clear();
                self.stroke_texts.clear();
            }
            Action::ToggleCase if self.grid_helpcode.editing => {
                self.grid_helpcode.uppercase = !self.grid_helpcode.uppercase;
            }
            Action::AppendGroup(code) | Action::AppendExact(code) if self.grid_helpcode.editing => {
                let exact = matches!(action, Action::AppendExact(_));
                let letters = if exact && code.is_ascii_lowercase() {
                    char::from(code).to_string()
                } else if !exact {
                    group_letters(self.grid, code).to_owned()
                } else {
                    String::new()
                };
                if letters.is_empty() {
                    return KeyResult::unhandled();
                }
                if self.grid_helpcode.slots.len() == 2 {
                    return KeyResult::handled()
                        .with_diagnostic(Some(diagnostics::GRID_HELPCODE_LIMIT.to_owned()));
                }
                self.grid_helpcode.slots.push(GridHelpcodeSlot {
                    letters,
                    exact: exact.then_some(code),
                    uppercase: self.grid_helpcode.uppercase,
                });
                refresh = true;
            }
            Action::Choose(value) if self.grid_helpcode.editing => {
                if !self
                    .grid_helpcode
                    .snapshot(Availability::Available)
                    .choices
                    .contains(&value)
                {
                    return KeyResult::unhandled();
                }
                // 先核验完整解释，再原子锁定；大小写也必须与槽一致。
                for (slot, letter) in self.grid_helpcode.slots.iter_mut().zip(value.bytes()) {
                    slot.exact = Some(letter.to_ascii_lowercase());
                }
                refresh = true;
            }
            Action::End if self.grid_helpcode.editing => {
                self.grid_helpcode.editing = false;
            }
            Action::Clear => {
                refresh = self.grid_helpcode.active();
                self.grid_helpcode.clear();
            }
            _ => return KeyResult::unhandled(),
        }
        if refresh {
            self.refresh();
        } else {
            self.grid_helpcode.changed();
        }
        KeyResult::handled()
    }

    pub(super) fn rank_grid_helpcode(&self, candidates: &mut Vec<WordItem>) {
        if let Some(table) = &self.grid_helpcode_table {
            self.grid_helpcode.rank(candidates, table);
        }
    }
}
