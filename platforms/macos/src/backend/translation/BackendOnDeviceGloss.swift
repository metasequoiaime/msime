// The macOS 26 SDK is the first with TranslationSession(installedSource:target:), the only way to get a session outside a SwiftUI view, and Swift 6.2 is the first compiler that ships with it. An older toolchain builds MSIMEBackend without this file, and InputController.mm finds the weak-imported entry point null.
#if compiler(>=6.2) && canImport(Translation)
import Foundation
import Translation
import os

// Candidate glosses from Apple's on-device translation models, for the Chinese candidates the packaged offline dictionaries leave empty. Only a language pair the user already downloaded in System Settings is used: nothing here starts a download, and a pair that is merely supported is skipped until it is installed. Translation runs on this Mac, so the candidates never leave it.
@available(macOS 26, *)
@MainActor
private enum BackendOnDeviceGloss {
  static let notification = Notification.Name("MSIMEBackendOnDeviceTranslationsDidArrive")
  private static let source = Locale.Language(identifier: "zh-Hans")
  // Asking whether a pair is installed is a round trip to the translation service, so a pair found missing is not asked about again for this long. Downloading one in System Settings takes effect within it.
  private static let recheck: Duration = .seconds(30)
  private static var sessions: [String: TranslationSession] = [:]
  private static var missing: [String: ContinuousClock.Instant] = [:]
  // One word in flight per language, taken from the newest page. The translation service works through its requests one at a time, about half a second per candidate and two to three seconds while it reloads a model that sat idle, and it keeps working through a batch after the task that asked for it is cancelled. A whole page as one batch therefore showed nothing for three to five seconds, and while the user typed on, the pages in between still had to finish before the one on screen started. Asking word by word, in page order, puts the first candidate's gloss up as soon as it alone is done and lets a newer page take over after at most one word.
  private static var busy: Set<String> = []
  private static var queued: [String: [String]] = [:]
  // The word each language is translating right now. A newer page that still shows it would otherwise queue it again behind itself and pay for it twice; its reply is on the way and every controller hears it.
  private static var inFlight: [String: String] = [:]
  private static let log = Logger(subsystem: BackendEdition.inputMethodBundleIdentifier, category: "translation")

  static func fetch(words: [String], targets: [String]) {
    for code in targets {
      let waiting = inFlight[code].map { word in words.filter { $0 != word } } ?? words
      queued[code] = waiting.isEmpty ? nil : waiting
      pump(code)
    }
  }

  private static func pump(_ code: String) {
    guard !busy.contains(code), queued[code] != nil else { return }
    busy.insert(code)
    Task { @MainActor in
      defer {
        busy.remove(code)
        pump(code)
      }
      guard let session = await session(for: code) else {
        // Not installed: drop the page rather than asking again the moment this returns.
        queued[code] = nil
        return
      }
      while var words = queued[code], !words.isEmpty {
        let word = words.removeFirst()
        queued[code] = words.isEmpty ? nil : words
        let started = ContinuousClock.now
        let response: TranslationSession.Response
        inFlight[code] = word
        defer { inFlight[code] = nil }
        do {
          response = try await session.translate(word)
        } catch {
          // The model was removed or the service restarted. The next page asks whether the pair is still installed.
          sessions[code] = nil
          queued[code] = nil
          log.log("on_device_gloss_failed target=\(code, privacy: .public)")
          return
        }
        let elapsed = ContinuousClock.now - started
        // Counts and timings only, never the candidate.
        log.log("on_device_gloss target=\(code, privacy: .public) ms=\(elapsed.components.seconds * 1000 + elapsed.components.attoseconds / 1_000_000_000_000_000) chars=\(word.count) waiting=\(queued[code]?.count ?? 0)")
        // A gloss equal to the word itself (a place name the model keeps in kanji, say) tells the reader nothing. It still goes back, empty, so the controller remembers the word as answered instead of asking on every keystroke.
        let gloss = response.targetText == word ? "" : response.targetText
        NotificationCenter.default.post(name: notification, object: nil, userInfo: ["target": code, "translations": [word: gloss]])
      }
    }
  }

  private static func session(for code: String) async -> TranslationSession? {
    if let session = sessions[code] { return session }
    if let checked = missing[code], ContinuousClock.now - checked < recheck { return nil }
    let target = Locale.Language(identifier: code)
    let status = await LanguageAvailability().status(from: source, to: target)
    // Only a pair the user can still download is worth pointing at; an unsupported one has nothing to offer.
    recordDownloadable(code, status == .supported)
    guard status == .installed else {
      missing[code] = .now
      return nil
    }
    missing[code] = nil
    let session = TranslationSession(installedSource: source, target: target)
    sessions[code] = session
    return session
  }

  // The settings app runs in another process and cannot ask the translation service on this one's behalf, so the pairs found downloadable but not downloaded are left in this input method's defaults domain for it to read with `defaults read`. A comma-separated string rather than an array, so that reader prints the value verbatim.
  static let downloadableDefaultsKey = "MSIMEOnDeviceTranslationDownloadableLanguages"

  private static func recordDownloadable(_ code: String, _ downloadable: Bool) {
    let defaults = UserDefaults.standard
    var codes = Set((defaults.string(forKey: downloadableDefaultsKey) ?? "").split(separator: ",").map(String.init))
    let changed = downloadable ? codes.insert(code).inserted : codes.remove(code) != nil
    guard changed else { return }
    if codes.isEmpty {
      defaults.removeObject(forKey: downloadableDefaultsKey)
    } else {
      defaults.set(codes.sorted().joined(separator: ","), forKey: downloadableDefaultsKey)
    }
  }
}

// Called on the main thread by InputController.mm with at most one page of Chinese candidates and the user's target languages. Results arrive as MSIMEBackendOnDeviceTranslationsDidArrive on the main thread, one notification per language.
@_cdecl("MSIMEFetchOnDeviceCandidateGlosses")
public func msimeFetchOnDeviceCandidateGlosses(_ wordsJSON: UnsafePointer<CChar>, _ targetsJSON: UnsafePointer<CChar>) {
  guard #available(macOS 26, *) else { return }
  guard let words = try? JSONDecoder().decode([String].self, from: Data(String(cString: wordsJSON).utf8)),
        let targets = try? JSONDecoder().decode([String].self, from: Data(String(cString: targetsJSON).utf8)),
        !words.isEmpty, words.count <= 32, !targets.isEmpty, targets.count <= 2 else { return }
  MainActor.assumeIsolated { BackendOnDeviceGloss.fetch(words: words, targets: targets) }
}
#endif
