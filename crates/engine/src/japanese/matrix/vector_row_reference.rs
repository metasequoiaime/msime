use super::*;

// 固定 33a5a3f46 的 Vec 行正文，供独立历史查询对照。
/// 行未超限时保留插入顺序，首次超限后按成本稳定维护前八项。
#[derive(Default)]
pub(super) struct Row {
    pub(super) nodes: Vec<Node>,
    ranked: bool,
}

impl Row {
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(MAX_NODES_PER_ROW),
            ranked: false,
        }
    }

    pub(super) fn extend(&mut self, prefix: &str, suffix: &str, cost: i64, right_id: u16) {
        if self.nodes.len() < MAX_NODES_PER_ROW {
            self.nodes.push(Node {
                text: join_text(prefix, suffix),
                cost,
                right_id,
            });
            return;
        }
        if !self.ranked {
            // 第九项即使败选，原批量截断也会把已有八项稳定排序。
            self.nodes.sort_by_key(|node| node.cost);
            self.ranked = true;
        }
        if cost >= self.nodes.last().expect("full row").cost {
            return;
        }
        self.nodes.pop();
        // 后到的同分项排在既有项之后，保留原稳定排序的优先级。
        let index = self.nodes.partition_point(|node| node.cost <= cost);
        self.nodes.insert(
            index,
            Node {
                text: join_text(prefix, suffix),
                cost,
                right_id,
            },
        );
    }
}
