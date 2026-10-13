import Foundation
import XCTest
@testable import MSIMEBackend

final class BackendBoundedFileReaderTests: XCTestCase {
  func testExactLimitAndOversize() throws {
    let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: file) }
    let data = Data(repeating: 0x41, count: 64 * 1024)
    try data.write(to: file)
    XCTAssertEqual(try BackendBoundedFileReader.read(from: file, maximumBytes: data.count), data)
    XCTAssertThrowsError(try BackendBoundedFileReader.read(from: file, maximumBytes: data.count - 1)) { error in
      XCTAssertEqual(error as? BackendBoundedFileReader.Failure, .tooLarge)
    }
  }

  func testRejectsLeafSymlink() throws {
    #if canImport(Darwin)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let file = root.appendingPathComponent("file")
    let linked = root.appendingPathComponent("linked")
    try Data("synthetic".utf8).write(to: file)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: file)
    XCTAssertThrowsError(try BackendBoundedFileReader.read(from: linked, maximumBytes: 64))
    #endif
  }
}
