import Foundation

private typealias NoticeByte = UInt8

@_silgen_name("msime_client_notices")
private func msimeClientNotices(_ request: UnsafePointer<NoticeByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_notice_dismiss")
private func msimeClientNoticeDismiss(_ request: UnsafePointer<NoticeByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientNoticeStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// A notice from GET /v1/notices?channel=app&platform=ios, through client-core (msime_client_notices), which caches the feed for a minute and leaves out the ones the user dismissed.
struct AppNotice: Identifiable, Equatable, Sendable {
  let id: String
  let title: String
  /// Simple Markdown.
  let body: String
}

enum AppNotices {
  /// The app's own directory for the cached feed and the dismissed ids; the keyboard never shows notices.
  static var directory: URL? {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
      .appendingPathComponent("MSIME/notices", isDirectory: true)
  }

  /// Blocks on the network at most once a minute: call off the main thread when the app home opens.
  static func load() -> [AppNotice] {
    guard let directory else { return [] }
    guard let value = call(msimeClientNotices, ["directory": directory.path, "platform": "ios", "channel": "app"]) as? [String: Any],
          let items = value["items"] as? [[String: Any]] else { return [] }
    return items.compactMap { item in
      guard let id = item["id"] as? String, let title = item["title"] as? String, let body = item["body"] as? String else { return nil }
      return AppNotice(id: id, title: title, body: body)
    }
  }

  /// Remembered locally, per notice id.
  static func dismiss(_ id: String) {
    guard let directory else { return }
    call(msimeClientNoticeDismiss, ["directory": directory.path, "id": id])
  }

  /// The body for a Text view: Markdown inline syntax with line breaks kept. Raw HTML is never interpreted, images are never loaded (an image becomes a link to it), and only http, https and mailto links survive; the system opens them outside the app.
  static func attributed(_ markdown: String) -> AttributedString {
    let options = AttributedString.MarkdownParsingOptions(allowsExtendedAttributes: false,
                                                          interpretedSyntax: .inlineOnlyPreservingWhitespace,
                                                          failurePolicy: .returnPartiallyParsedIfPossible)
    var text = (try? AttributedString(markdown: markdown, options: options)) ?? AttributedString(markdown)
    for run in text.runs {
      var link = run.link
      if let image = run.imageURL {
        text[run.range].imageURL = nil
        link = link ?? image
      }
      if let url = link, !["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") { link = nil }
      text[run.range].link = link
    }
    return text
  }

  @discardableResult
  private static func call(_ function: (UnsafePointer<NoticeByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                           _ request: [String: Any]) -> Any? {
    guard let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
    let raw = body.withUnsafeBytes { function($0.bindMemory(to: NoticeByte.self).baseAddress, UInt(body.count)) }
    guard let raw else { return nil }
    defer { msimeClientNoticeStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(bytes: raw, count: strlen(raw))) as? [String: Any],
          envelope["ok"] as? Bool == true else { return nil }
    return envelope["value"]
  }
}
