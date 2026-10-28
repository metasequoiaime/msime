//! 使用合成词库验证商业云候选契约和上屏后联想，不读取用户配置或输入历史。
use msime_client_core::cloud::candidates::{
    build_request, parse_response, CloudCandidatePreferences,
};
use msime_engine::{CandidateSource, RuntimePaths, Session, SessionOptions};
use serde_json::{json, Value};
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "transaction".into());
    let config: Result<CloudCandidatePreferences, _> =
        serde_json::from_value(json!({"source":"baidu", "cache_enabled":false}));
    if mode == "describe-baidu" {
        let config = config?;
        println!(
            "{}",
            serde_json::to_string(
                &build_request(&config, "nihao", false).ok_or("request unavailable")?
            )?
        );
        return Ok(());
    }
    if mode == "parse-baidu" {
        let mut body = Vec::new();
        std::io::stdin().take(262145).read_to_end(&mut body)?;
        let request = build_request(&config?, "nihao", false).ok_or("request unavailable")?;
        let candidate = parse_response(&request, &body).ok_or("no valid cloud candidate")?;
        println!("{}", json!({"source":"baidu", "candidate":candidate}));
        return Ok(());
    }
    if mode != "transaction" {
        return Err("unknown probe mode".into());
    }
    let directory = tempfile::tempdir()?;
    let path = directory.path().to_path_buf();
    let database = rusqlite::Connection::open(path.join("msime-pinyin.db"))?;
    database.execute_batch(
        "CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_c VALUES('chi','c','吃',1000);
        CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_2_c VALUES('chi''fan','cf','吃饭',100);",
    )?;
    drop(database);
    let mut options = SessionOptions::new(RuntimePaths {
        resources: path.clone(),
        user_data: path.clone(),
        cache: path.clone(),
        dictionaries: path,
    });
    options.helpcode = false;
    options.learning = false;
    options.personal_context = false;
    options.next_word_prediction = true;
    let mut session = Session::new(options)?;
    for byte in b"chi" {
        session.character(*byte, false);
    }
    let index = session
        .snapshot()
        .candidates
        .iter()
        .position(|row| row.word == "吃")
        .ok_or("seed missing")?;
    let commit = session.select(index).commit;
    let local: Vec<_> = session
        .snapshot()
        .candidates
        .iter()
        .map(|row| row.word.to_string())
        .collect();
    let mut prediction_mode = Value::Null;
    let mut applied = false;
    let mut suffix = None;
    if let Some(query) = session.online_query() {
        prediction_mode = Value::String(format!("{:?}", query.mode));
        applied = session.apply_online_candidates(
            &query,
            &["米饭".into()],
            CandidateSource::CloudSuggestion,
        );
        if let Some(index) = session
            .snapshot()
            .candidates
            .iter()
            .position(|row| row.word == "米饭")
        {
            suffix = session.select(index).commit;
        }
    }
    println!(
        "{}",
        json!({"baidu_supported":config.is_ok(), "seed_commit":commit,
        "local_predictions":local, "online_mode":prediction_mode, "cloud_applied":applied, "suffix_commit":suffix})
    );
    Ok(())
}
