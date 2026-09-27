#!/usr/bin/env xcrun swift

// Ask the input source registry whether a bundle identifier is actually there, and enabled.
//
// --register-input-source returning 0 does not mean it worked. After a bundle is replaced, the first call
// can succeed against the previous one's LaunchServices entry and report nothing wrong while the new
// bundle is registered nowhere - which looks exactly like a working install until the user opens the
// input menu and finds it missing.
//
// When it is missing, there are two causes and they need telling apart, because one is a bug in the build
// and the other cannot be fixed from here at all:
//
//   1. The bundle is ad-hoc signed. TISRegisterInputSource accepts it and the source never appears.
//   2. The bundle identifier is new to this login session. A bundle identifier that was not already in the
//      input source list when the session began cannot be added to it, whatever the bundle contains.
//
// The second one is measured, not assumed. Take the installed input method that does work, copy it, give
// the copy a fresh bundle identifier, re-sign it with the same Developer ID and register it: it fails
// exactly the same way - TISRegisterInputSource returns noErr and TISCreateInputSourceList never lists it.
// Restarting imklaunchagent, re-registering through lsregister and rescanning the user domain all leave it
// absent. The one input method that is listed is the one whose identifier predates the session.
//
// So an absent source is only evidence of a broken bundle when the identifier is one this session has seen
// before. This script prints the session start so that is checkable rather than guessed at.
//
// A registered source that is not enabled is a third state, and collapsing it into the two above is what
// this script used to do - it exited 1 for any disabled source, which made install.sh report an input method
// that types fine as absent from the session and tell the user to log out, which changes nothing. An input
// method with modes is several sources: an umbrella source that is not select-capable, and one source per
// mode. What the user picks from the input menu is a mode, so the question that decides whether the install
// is usable is whether any *select-capable* source is enabled, not whether all of them are.
//
// A mode that stays disabled is not ours to fix. An input source of type keyboard input mode cannot be
// enabled from a process on macOS 27.0: TISEnableInputSource returns noErr and the mode stays disabled.
// Measured against Apple's own com.apple.inputmethod.SCIM.Shuangpin, whose parent input method is enabled
// and which behaves identically, while com.apple.keylayout.US in the same session enables and disables
// normally - so it is the source type that is refused, not the API, the bundle or the signature.
//
// Exit codes, because install.sh has to tell these apart and only the first two are a good install:
//
//   0  every registered source is enabled
//   2  usable - a select-capable source is enabled - with some other source left disabled
//   1  not usable: absent from the registry, or present with nothing select-capable enabled
//
// Usage: check_input_source.swift <bundle identifier> [bundle path]

import Carbon
import Foundation

guard CommandLine.arguments.count == 2 || CommandLine.arguments.count == 3 else {
    FileHandle.standardError.write("usage: check_input_source.swift <bundle identifier> [bundle path]\n".data(using: .utf8)!)
    exit(2)
}
let identifier = CommandLine.arguments[1]
let bundlePath = CommandLine.arguments.count == 3 ? CommandLine.arguments[2] : nil

func run(_ launchPath: String, _ arguments: [String]) -> String {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: launchPath)
    process.arguments = arguments
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = pipe
    guard (try? process.run()) != nil else { return "" }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    return String(data: data, encoding: .utf8) ?? ""
}

/// Whether the bundle carries a real signing identity, which is the half of an absent source that is ours
/// to fix. An ad-hoc signature has no Authority line.
func adHocSigned(_ path: String) -> Bool {
    let output = run("/usr/bin/codesign", ["-dv", "--verbose=2", path])
    return !output.contains("Authority=")
}

/// When this login session began. An identifier first registered after this cannot enter the session's
/// input source list.
func sessionStart() -> String {
    let output = run("/bin/ps", ["-Ao", "pid,lstart,comm"])
    for line in output.split(separator: "\n") where line.contains("/loginwindow") {
        let fields = line.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: true)
        if fields.count == 2 {
            return fields[1].replacingOccurrences(of: "/System/Library/CoreServices/loginwindow.app/Contents/MacOS/loginwindow", with: "").trimmingCharacters(in: .whitespaces)
        }
    }
    return "unknown"
}

let filter = [kTISPropertyBundleID as String: identifier] as CFDictionary
guard let sources = TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource],
      !sources.isEmpty else {
    var message = """
        \(identifier) is not in the input source registry. --register-input-source may still have exited 0 \
        against a stale entry for the bundle it replaced.

        """
    if let path = bundlePath, adHocSigned(path) {
        message += """
            \(path) is ad-hoc signed, which is enough on its own to keep it out of the registry. Sign it \
            with a Developer ID Application identity - platforms/macos/scripts/install.sh does this.

            """
    } else {
        message += """
            The signature is not the problem. If this bundle identifier was first registered after the \
            current login session began (\(sessionStart())), it cannot enter this session's input source \
            list whatever the bundle contains, and only logging out and back in will add it. See the \
            comment at the top of this script for how that was measured.

            """
    }
    FileHandle.standardError.write(message.data(using: .utf8)!)
    exit(1)
}

func flag(_ source: TISInputSource, _ key: CFString) -> Bool {
    guard let value = TISGetInputSourceProperty(source, key) else { return false }
    return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(value).takeUnretainedValue())
}

var disabled: [String] = []
var selectable: [String] = []
for source in sources {
    guard let raw = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else { continue }
    let id = Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
    let enabled = flag(source, kTISPropertyInputSourceIsEnabled)
    // The umbrella source of an input method with modes is not select-capable, so say which lines are
    // which rather than leaving the reader to wonder why an enabled source is not in the input menu.
    let selectCapable = flag(source, kTISPropertyInputSourceIsSelectCapable)
    print("\(id): \(enabled ? "enabled" : "disabled")\(selectCapable ? "" : " (not selectable)")")
    if !enabled { disabled.append(id) }
    if enabled && selectCapable { selectable.append(id) }
}

if selectable.isEmpty {
    FileHandle.standardError.write("""
        \(identifier) is registered, but none of its select-capable input sources is enabled, so it cannot \
        be chosen from the input menu. Add it in System Settings > Keyboard > Text Input > Input Sources.

        """.data(using: .utf8)!)
    exit(1)
}
if !disabled.isEmpty {
    FileHandle.standardError.write("""
        registered but not enabled: \(disabled.joined(separator: ", "))
        \(identifier) is installed and selectable as \(selectable.joined(separator: ", ")), so this is not a \
        signing or registration failure, and logging out will not change it. On \
        \(ProcessInfo.processInfo.operatingSystemVersionString) an input source of type keyboard input mode \
        cannot be enabled from a process: TISEnableInputSource returns noErr and the mode stays disabled, \
        for Apple's own modes as much as for this one. Add the missing entry by hand in \
        System Settings > Keyboard > Text Input > Input Sources.

        """.data(using: .utf8)!)
    exit(2)
}
