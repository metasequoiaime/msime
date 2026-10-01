import Foundation

@_cdecl("MSIMEStartTelemetry")
public func msimeStartTelemetry() {
  Task { await BackendTelemetryClient.shared.recordFirstLaunch() }
  NSSetUncaughtExceptionHandler { exception in
    BackendTelemetryClient.persistCrash(message: exception.reason ?? exception.name.rawValue,
                                        stack: exception.callStackSymbols.joined(separator: "\n"))
  }
}

/// Synchronous account state used by native settings surfaces that only need to
/// decide whether to offer sign-in. The access token itself never crosses this
/// bridge.
@_cdecl("MSIMEBackendAccountSignedIn")
public func msimeBackendAccountSignedIn() -> Bool {
  if ((try? BackendDesktopSessionFile().load()) ?? nil) != nil { return true }
  return ((try? BackendAnonymousAccount.sessionStorage().load()) ?? nil) != nil
}

/// Warm the anonymous account used by keyboard-only services after the input
/// source is first activated. Failure is deliberately best effort: typing must
/// never wait for account setup or be blocked by a network outage.
private actor AnonymousAccountBootstrap {
  static let shared = AnonymousAccountBootstrap()
  private var attempted = false

  func runOnce() async {
    guard !attempted else { return }
    attempted = true
    let signedIn = BackendAccountSession.shared
    if (try? await signedIn.accessToken()) != nil { return }
    let anonymous = BackendAnonymousAccount.session
    if (try? await anonymous.accessToken()) != nil { return }
    _ = try? await BackendAnonymousAccount.ensureSignedIn(session: anonymous, client: BackendAccountClient())
  }
}

@_cdecl("MSIMEEnsureAnonymousAccount")
public func msimeEnsureAnonymousAccount() {
  Task { await AnonymousAccountBootstrap.shared.runOnce() }
}
