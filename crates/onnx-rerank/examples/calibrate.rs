//! 量一个导出的模型在这台机器上打一页重负载要多久（`OnnxScorer::calibrate`），宿主据此在大小两个模型之间选。
//!
//! `calibrate <模型目录> [线程数] [次数]`：模型目录里是 `export_step.py` 导出的 `lm_step.ort`、`model.json`、`vocab.json`。要开 `runtime` 特性。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let directory = std::path::Path::new(
        args.first()
            .ok_or("usage: calibrate <model dir> [threads] [rounds]")?,
    );
    let threads = args.get(1).map_or(Ok(2), |t| t.parse())?;
    let rounds = args.get(2).map_or(Ok(5), |r| r.parse())?;
    let read = |name: &str| std::fs::read_to_string(directory.join(name));
    let model = std::fs::read(directory.join("lm_step.ort"))?;
    let mut scorer = msime_onnx_rerank::OnnxScorer::load(
        &model,
        &read("model.json")?,
        &read("vocab.json")?,
        threads,
    )?;
    for round in 1..=rounds {
        println!("round {round}: {:.2} ms", scorer.calibrate());
    }
    Ok(())
}
