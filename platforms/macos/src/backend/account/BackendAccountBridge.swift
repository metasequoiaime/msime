import Foundation

/// Stable opaque identity for one saved login. This is shared by the backend
/// notification and the native input method's cache scope; neither side needs
/// to expose the account ID or session UUID to the other.
func backendAccountTranslationMarker(userID: String, sessionID: UUID?) -> UInt64 {
  let identity = "\(userID)|\(sessionID?.uuidString ?? "")"
  var hash: UInt64 = 14695981039346656037
  for byte in identity.utf8 {
    hash ^= UInt64(byte)
    hash &*= 1099511628211
  }
  return hash
}

/// Synchronous account state used by native settings surfaces that only need to
/// decide whether to offer sign-in. The access token itself never crosses this
/// bridge.
@_cdecl("MSIMEBackendAccountSignedIn")
public func msimeBackendAccountSignedIn() -> Bool {
  if ((try? BackendDesktopSessionFile().load()) ?? nil) != nil { return true }
  return ((try? BackendAnonymousAccount.sessionStorage().load()) ?? nil) != nil
}

/// Opaque, process-local scope for account candidate glosses. The input method only needs to
/// distinguish login generations; it never receives the account ID or session UUID itself.
@_cdecl("MSIMEBackendAccountTranslationMarker")
public func msimeBackendAccountTranslationMarker() -> UInt64 {
  guard let saved = (try? BackendDesktopSessionFile().load()) ?? nil else { return 0 }
  return backendAccountTranslationMarker(userID: saved.tokens.user.id, sessionID: saved.sessionID)
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
