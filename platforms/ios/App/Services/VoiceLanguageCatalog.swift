// 由共享 Rust 语音语言目录导出；通过 export_voice_language_catalog 更新。
enum VoiceLanguageCatalog {
  static let mobile: [(id: String, title: String)] = [
    ("zh-cn", "普通话"),
    ("yue", "粤语"),
    ("en", "英语"),
    ("auto", "普通话 + 英语"),
  ]
}
