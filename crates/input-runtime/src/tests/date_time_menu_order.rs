use super::*;

/// 挂到完整输入会话的逐键打分器：池大小覆盖增强模式，分数不改变词典首选。
struct DateMenuScorer(usize);

impl CandidateScorer for DateMenuScorer {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        assert!(!candidates.contains(&"📅日期"), "入口不能参与句子比较");
        vec![-5.0; candidates.len()]
    }

    fn sentence_pool(&self) -> usize {
        self.0
    }
}

fn date_runtime(pool: usize) -> (tempfile::TempDir, Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let mut options = real_engine_options(directory.path());
    options.mixed_english = false;
    rusqlite::Connection::open(
        std::path::Path::new(&options.dictionaries).join(msime_engine::assets::MAIN_DICTIONARY),
    )
    .unwrap()
    .execute_batch(
        "CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_1_r VALUES('ri','r','日',100000);
         CREATE TABLE tbl_1_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_1_q VALUES('qi','q','期',100000);
         INSERT INTO tbl_1_q VALUES('qi','q','企',80000);
         CREATE TABLE tbl_2_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_2_r VALUES('ri''qi','rq','日期',200000);
         INSERT INTO tbl_2_r VALUES('ri''qi','rq','日企',50000);",
    )
    .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 6).unwrap();
    runtime.focus(true).unwrap();
    if pool != 0 {
        runtime.set_candidate_scorer(Some(Box::new(DateMenuScorer(pool))));
    }
    (directory, runtime)
}

/// #11513：独立查询不挂逐键重排器，必须通过 runtime 打字，才能覆盖整句池之后的读法降位。
#[test]
fn date_time_menu_stays_after_the_first_date_with_a_pooled_scorer() {
    for pool in [0, 9, 32] {
        for keys in ["riqi", "rq"] {
            let (_directory, mut runtime) = date_runtime(pool);
            type_characters(&mut runtime, keys);
            let rows = runtime.all_candidates().candidates;
            let date = rows
                .iter()
                .position(|row| row.text.starts_with('2') && row.text.contains('年'))
                .expect("第一条年月日日期值");
            let menu = rows
                .iter()
                .position(|row| row.text == "📅日期")
                .expect("日期入口");
            assert_eq!(menu, date + 1, "pool={pool}, keys={keys}, rows={rows:?}");
        }
    }
}

#[test]
fn date_time_menu_after_reranking_opens_formats_and_commits_the_selected_date() {
    for pool in [0, 9, 32] {
        for keys in ["riqi", "rq"] {
            let (_directory, mut runtime) = date_runtime(pool);
            type_characters(&mut runtime, keys);
            let rows = runtime.all_candidates().candidates;
            let menu = rows
                .iter()
                .find(|row| row.text == "📅日期")
                .expect("日期入口");
            // 选中经过重排后的入口仍然换格式页，不把标签上屏；再选格式仍提交对应日期。
            let opened = runtime
                .dispatch(Action::SelectAnyCandidate(menu.id))
                .unwrap();
            assert!(opened.handled);
            assert!(opened.commit.is_none());
            let formats = runtime.all_candidates().candidates;
            assert!(formats.len() > 6, "{formats:?}");
            assert!(!formats.iter().any(|row| row.text == "📅日期"));
            let expected = formats[1].text.clone();
            let committed = runtime
                .dispatch(Action::SelectAnyCandidate(formats[1].id))
                .unwrap();
            assert_eq!(
                committed.commit.as_deref(),
                Some(expected.as_str()),
                "pool={pool}, keys={keys}, result={committed:?}"
            );
        }
    }
}
