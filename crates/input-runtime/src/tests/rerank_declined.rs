use super::*;
use msime_engine::ordering::{rerank_pick, OrderRow, OrderRowSource, DICTIONARY_SOURCES};
use std::cell::Cell;

struct CountedRows<'a> {
    rows: &'a [OrderRow<'a>],
    reads: Cell<usize>,
}

impl<'a> OrderRowSource<'a> for CountedRows<'a> {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> OrderRow<'a> {
        self.reads.set(self.reads.get() + 1);
        self.rows[index]
    }
}

fn row(text: &str) -> OrderRow<'_> {
    OrderRow {
        text,
        source: LATTICE_SOURCE,
        answers_key: true,
        corrected: false,
    }
}

fn model() -> Reranker {
    Reranker::new(std::sync::Arc::new(favouring_model(&['甲', '乙'], &['乙'])))
}

#[test]
fn declined_rerank_skips_projection_allocations_and_redundant_row_reads() {
    let context = "合成上文".repeat(128);
    for count in [129, 2, 128, 1024] {
        for scheme in [0, 1] {
            for source in [0, 1, 3, 4, 5, 6, 7, 8, 12, 13] {
                let mut rows = vec![row("甲"); count];
                rows[0].source = source;
                if source == LATTICE_SOURCE {
                    rows[0].answers_key = false;
                }
                let counted = CountedRows {
                    rows: &rows,
                    reads: Cell::new(0),
                };
                let mut reranker = model();
                let (picked, allocations, bytes) = allocations::count_bytes(|| {
                    rerank_pick(&mut reranker, &context, scheme, false, &counted)
                });
                assert_eq!(picked, None);
                assert_eq!(
                    (allocations, bytes),
                    (0, 0),
                    "rows={count} scheme={scheme} source={source}"
                );
                // 全拼词典首选仍需查看整表纠错；其它已拒绝的首选只读取一行。
                let budget = if scheme == 0 && DICTIONARY_SOURCES.contains(&source) {
                    count + 1
                } else {
                    1
                };
                assert!(
                    counted.reads.get() <= budget,
                    "rows={count} scheme={scheme} source={source}: {} > {budget}",
                    counted.reads.get()
                );
            }
        }
    }
}

#[test]
fn tail_corrections_still_remove_only_quanpin_dictionary_deference() {
    for count in [2, 129, 1024] {
        for source in DICTIONARY_SOURCES {
            let mut rows = vec![
                OrderRow {
                    answers_key: false,
                    ..row("甲")
                };
                count
            ];
            rows[0] = OrderRow {
                source,
                ..row("甲")
            };
            rows[1] = row("乙");
            rows[count - 1].corrected = true;
            // 即使纠错行自己不参与比较，全拼仍按原规则撤销整表的词典豁免。
            assert_eq!(rerank_pick(&mut model(), "甲", 0, false, &rows), Some(1));
            assert_eq!(rerank_pick(&mut model(), "甲", 1, false, &rows), None);
        }
    }
}

#[test]
fn declined_rerank_leaves_the_warmed_model_ready_for_the_next_decision() {
    let model = std::sync::Arc::new(favouring_model(&['甲', '乙'], &['乙']));
    let compared = [row("甲"), row("乙")];
    let mut control = Reranker::new(model.clone());
    let mut skipped = Reranker::new(model);
    assert_eq!(
        rerank_pick(&mut control, "甲", 0, false, &compared),
        Some(1)
    );
    assert_eq!(
        rerank_pick(&mut skipped, "甲", 0, false, &compared),
        Some(1)
    );
    let mut declined = vec![row("甲"); 1024];
    for source in [0, 1, 3, 5, 6, 7, 8, 12] {
        declined[0].source = source;
        declined[0].answers_key = source != LATTICE_SOURCE;
        assert_eq!(
            rerank_pick(&mut skipped, &"合成上文".repeat(128), 0, false, &declined),
            None
        );
    }
    let baseline =
        allocations::count_bytes(|| rerank_pick(&mut control, "甲", 0, false, &compared));
    let after = allocations::count_bytes(|| rerank_pick(&mut skipped, "甲", 0, false, &compared));
    assert_eq!(after, baseline);
    assert_eq!(after.0, Some(1));
}

#[test]
fn shuangpin_corrected_leader_skips_model_preparation() {
    for count in [129, 2, 1024] {
        let mut rows = vec![row("乙"); count];
        rows[0] = OrderRow {
            corrected: true,
            ..row("甲")
        };
        let counted = CountedRows {
            rows: &rows,
            reads: Cell::new(0),
        };
        let mut model = model();
        let measured =
            allocations::count_bytes(|| rerank_pick(&mut model, "甲", 1, false, &counted));
        assert_eq!(measured, (None, 0, 0));
        assert_eq!(counted.reads.get(), 1);
        assert_eq!(rerank_pick(&mut model, "甲", 0, false, &rows), Some(1));
    }
}
