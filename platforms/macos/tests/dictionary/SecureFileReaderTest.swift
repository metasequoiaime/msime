import Foundation

@main enum SecureFileReaderTest {
  static func main() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-secure-reader-test-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    let target = root.appendingPathComponent("target.txt")
    let expected = Data("synthetic wordbook".utf8)
    try expected.write(to: target)
    let selected = root.appendingPathComponent("selected.txt")
    try FileManager.default.createSymbolicLink(at: selected, withDestinationURL: target)
    do {
      _ = try MacSecureFileReader.readData(from: selected, maximumBytes: 64 * 1024)
      fatalError("symlinked source was accepted")
    } catch { }
    guard try MacSecureFileReader.readData(from: target, maximumBytes: 64 * 1024) == expected else {
      fatalError("regular source was not read")
    }
  }
}
