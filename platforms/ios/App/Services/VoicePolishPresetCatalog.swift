// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。
enum VoicePolishPresetCatalog {
  static let builtIns: [(id: String, title: String)] = [
    ("cleanup", "精炼整理"),
    ("faithful", "忠实校对"),
    ("zh2en", "中翻英"),
    ("casual", "口语整理"),
  ]
}
