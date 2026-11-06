//! 十四／十八键网格辅助码的会话入口（`nine_key::grid_helpcode`）；九键没有接入。

use super::*;
use crate::helpcode::SharedKeymap;
use crate::GridHelpcodeOp;

impl Session {
    /// 放下分隔符、追加整组或确定字母、清掉辅助码。不接受时返回未处理，组字、候选修订号和待用触点都不变；接受了但没有变化（重复放下、两槽已满）时返回已处理，修订号也不变。放下分隔符后 `grid_key` 一律未处理，宿主把字母键改送 `Group`。
    pub fn grid_helpcode(&mut self, op: GridHelpcodeOp) -> KeyResult {
        let changed = match op {
            GridHelpcodeOp::Begin => self.grid_helpcode_table().and_then(|table| {
                let strict = self.input.engine.strict_quanpin_helpcode();
                self.nine_key.begin_grid_helpcode(table, strict)
            }),
            GridHelpcodeOp::Group(letter) => self.nine_key.append_grid_helpcode(letter, false),
            GridHelpcodeOp::Exact(letter) => self.nine_key.append_grid_helpcode(letter, true),
            GridHelpcodeOp::Clear => self.nine_key.clear_grid_helpcode(),
        };
        let Some(changed) = changed else {
            return KeyResult::unhandled();
        };
        if changed {
            self.candidates_may_change();
            self.input.pending_touch = None;
        }
        KeyResult::handled()
    }

    /// 开关打开、当前方案的辅助码打开并有非空码表时，放下分隔符要取定的那张表。
    fn grid_helpcode_table(&self) -> Option<SharedKeymap> {
        if !self.grid_helpcode || !self.input.helpcode_enabled() {
            return None;
        }
        self.input
            .helpcode_keymap
            .clone()
            .filter(|table| !table.is_empty())
    }

    /// 快照的 `grid_helpcode_available`：这次组字能用网格辅助码，宿主据此画「辅」角标、接上滑。
    pub(super) fn grid_helpcode_available(&self) -> bool {
        self.nine_key.grid_helpcode_ready() && self.grid_helpcode_table().is_some()
    }
}
