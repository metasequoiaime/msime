import Foundation

/// A small glyph drawn after a candidate: where it came from, and whether the user pinned it.
///
/// The Windows candidate window appends ☁️ to a cloud candidate and 🤖 to an AI one (`event_listener.cpp`), and draws a pinned word in its accent colour. iOS asks the user to opt in before either network source runs, so a suggestion that came back from Google or a language model should not look like a dictionary word. Symbols stand in for the emoji so the glyph takes the skin's colour, and each one carries a spoken form because a colour or icon alone says nothing to VoiceOver.
struct CandidateMarker: Equatable {
  let symbol: String
  let spoken: String

  /// Engine `CandidateSource` values, shared by every host through the runtime's view.
  static let cloudSource = 2
  static let cloudPendingSource = 15
  static let aiSource = 3
  /// AI 占位行（「…」等待中，「—」没有结果）：不是候选，画成次要色，不加标记（#6888）。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里留着，旧的库或数据里出现时仍按占位行画。
  static let aiPendingSource = 12

  /// 来源 15 是云候选等待期间的内部固定席位，移动端不把它当成候选绘制。
  static func isVisible(source: Int) -> Bool {
    source != cloudPendingSource
  }

  /// 把移动端显示序号映射回 Engine 候选列表索引。
  static func visibleIndices(sources: [Int]) -> [Int] {
    sources.indices.filter { isVisible(source: sources[$0]) }
  }

  /// 读屏念的占位行：「…」是还在等 AI 回答，「—」是 AI 没有给出结果。候选条和展开的候选网格共用（#6888）。
  static func aiPendingSpoken(_ text: String) -> String {
    text == "…" ? "AI 候选等待中" : "AI 候选没有结果"
  }

  static func pendingSpoken(_ text: String, markers: [CandidateMarker]) -> String {
    markers.contains { $0.spoken == "云候选加载中" } ? "云候选加载中" : aiPendingSpoken(text)
  }

  static func markers(source: Int, fixedPosition: Int) -> [CandidateMarker] {
    var markers: [CandidateMarker] = []
    if source == cloudSource || source == cloudPendingSource {
      let pending = source == cloudPendingSource
      markers.append(CandidateMarker(
        symbol: pending ? "" : "cloud", spoken: pending ? "云候选加载中" : "云候选"))
    }
    if source == aiSource { markers.append(CandidateMarker(symbol: "sparkles", spoken: "AI 候选")) }
    if fixedPosition > 0 {
      markers.append(CandidateMarker(symbol: "pin.fill", spoken: "已固定第 \(fixedPosition) 位"))
    }
    return markers
  }
}
