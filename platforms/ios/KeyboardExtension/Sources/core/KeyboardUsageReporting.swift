import Foundation

@_silgen_name("msime_ios_crash_record_path_set")
private func msimeIOSCrashRecordPathSet(_ path: UnsafePointer<CChar>?)
@_silgen_name("msime_ios_crash_handlers_install")
private func msimeIOSCrashHandlersInstall()

/// One usage-reporting session per keyboard presentation (UsageReporting). The file work runs on a utility queue so it never delays the keyboard appearing.
enum KeyboardUsageReporting {
  private static let queue = DispatchQueue(label: "app.msime.ios.keyboard.usage-reporting", qos: .utility)
  /// Read and written on `queue` only.
  private static var lastFlush: Date?
  private static var exceptionHandlerInstalled = false
  /// A keyboard is shown many times a day; its events wait in the App Group queue between sends.
  private static let flushInterval: TimeInterval = 15 * 60

  /// viewWillAppear. Sending needs Full Access; without it the app sends the queue the next time it opens.
  static func presented(fullAccess: Bool) {
    msimeIOSCrashHandlersInstall()
    if !exceptionHandlerInstalled {
      exceptionHandlerInstalled = true
      NSSetUncaughtExceptionHandler { exception in
        UsageReporting.recordCrash(message: "Uncaught \(exception.name.rawValue): \(exception.reason ?? "")",
                                   stack: exception.callStackSymbols.joined(separator: "\n"))
      }
    }
    queue.async {
      guard let path = UsageReporting.begin() else { msimeIOSCrashRecordPathSet(nil); return }
      path.withCString { msimeIOSCrashRecordPathSet($0) }
      guard fullAccess, lastFlush.map({ Date().timeIntervalSince($0) >= flushInterval }) ?? true else { return }
      lastFlush = Date()
      UsageReporting.flush()
    }
  }

  /// viewWillDisappear: the presentation ended normally.
  static func dismissed() {
    queue.async {
      msimeIOSCrashRecordPathSet(nil)
      UsageReporting.end()
    }
  }
}
