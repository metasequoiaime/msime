import Foundation
import XCTest
@testable import MSIMEBackend

final class SafePathTests: XCTestCase {
  func testAliasesRequireTheExactSystemTarget() {
    XCTAssertTrue(SafePath.isTrustedSystemAliasTarget("/tmp", target: "private/tmp"))
    XCTAssertTrue(SafePath.isTrustedSystemAliasTarget("/tmp", target: "/private/tmp"))
    XCTAssertTrue(SafePath.isTrustedSystemAliasTarget("/var", target: "private/var"))
    XCTAssertTrue(SafePath.isTrustedSystemAliasTarget("/var", target: "/private/./other/../var"))
    XCTAssertFalse(SafePath.isTrustedSystemAliasTarget("/tmp", target: "/Users/synthetic/outside"))
    XCTAssertFalse(SafePath.isTrustedSystemAliasTarget("/var", target: "/private/tmp"))
    XCTAssertFalse(SafePath.isTrustedSystemAliasTarget("/tmp/work", target: "/private/tmp/work"))
    XCTAssertFalse(SafePath.isTrustedSystemAliasTarget("/etc", target: "private/etc"))
  }

  func testOnlyTheListedSystemLinksAreTrusted() throws {
    #if os(macOS)
    XCTAssertTrue(SafePath.isTrustedSystemAlias("/var"))
    XCTAssertTrue(SafePath.isTrustedSystemAlias("/tmp"))
    // `/etc` 也是指向 `/private` 的系统链接，但它不在清单里。
    XCTAssertFalse(SafePath.isTrustedSystemAlias("/etc"))
    XCTAssertFalse(SafePath.isTrustedSystemAlias("/private/var"))
    XCTAssertFalse(SafePath.hasRefusedSymbolicLink(URL(fileURLWithPath: "/tmp/msime-safe-path-missing-\(UUID().uuidString)/below")))
    XCTAssertFalse(SafePath.hasRefusedSymbolicLink(FileManager.default.temporaryDirectory.appendingPathComponent("missing")))
    // 别名本身作为最后一级时要拒绝，与 `reject_symlinked_components` 一致。
    XCTAssertTrue(SafePath.hasRefusedSymbolicLink(URL(fileURLWithPath: "/tmp")))
    #else
    throw XCTSkip("the system aliases exist on macOS")
    #endif
  }

  func testRefusesAPlantedLinkAndAcceptsARealPath() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-safe-path-\(UUID().uuidString)", isDirectory: true)
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent("msime-safe-path-outside-\(UUID().uuidString)", isDirectory: true)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    try FileManager.default.createDirectory(at: root.appendingPathComponent("real", isDirectory: true), withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    let linked = root.appendingPathComponent("linked", isDirectory: true)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)
    // 名字与别名相同但不在根目录下的链接，不算系统别名。
    let fakeAlias = root.appendingPathComponent("tmp", isDirectory: true)
    try FileManager.default.createSymbolicLink(atPath: fakeAlias.path, withDestinationPath: "/private/tmp")

    XCTAssertFalse(SafePath.hasRefusedSymbolicLink(root.appendingPathComponent("real/state.json")))
    XCTAssertFalse(SafePath.hasRefusedSymbolicLink(root.appendingPathComponent("missing/below")))
    XCTAssertTrue(SafePath.hasRefusedSymbolicLink(linked))
    XCTAssertTrue(SafePath.hasRefusedSymbolicLink(linked.appendingPathComponent("missing/below")))
    XCTAssertTrue(SafePath.hasRefusedSymbolicLink(fakeAlias.appendingPathComponent("below")))
  }
}
