import Foundation
import XCTest

final class BoundedFileReaderTests: XCTestCase {
  func testRejectsAFileThatGrowsPastTheConfiguredLimit() throws {
    let file = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: file) }
    try Data(repeating: 0x5A, count: 1_025).write(to: file)

    XCTAssertThrowsError(try BoundedFileReader.read(from: file, maximumBytes: 1_024)) { error in
      XCTAssertEqual(error as? BoundedFileReader.Failure, .tooLarge)
    }
  }

  func testReadsAFileAtTheConfiguredLimit() throws {
    let file = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: file) }
    let expected = Data(repeating: 0xA5, count: 1_024)
    try expected.write(to: file)

    XCTAssertEqual(try BoundedFileReader.read(from: file, maximumBytes: expected.count), expected)
  }
}
