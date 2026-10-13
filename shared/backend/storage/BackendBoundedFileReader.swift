import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// Bounded reads for backend state files. Opening without following the final symlink
/// also protects a file replaced after the caller checked its path.
enum BackendBoundedFileReader {
  enum Failure: Error, Equatable {
    case invalidLimit
    case unreadable
    case tooLarge
  }

  static func read(from url: URL, maximumBytes: Int) throws -> Data {
    guard maximumBytes >= 0 else { throw Failure.invalidLimit }
    #if canImport(Darwin)
    let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else { throw Failure.unreadable }
    defer { close(descriptor) }
    var data = Data()
    data.reserveCapacity(min(maximumBytes, 64 * 1024))
    var buffer = [UInt8](repeating: 0, count: maximumBytes < 64 * 1024 ? maximumBytes + 1 : 64 * 1024)
    while true {
      let count = buffer.withUnsafeMutableBytes { bytes in
        Darwin.read(descriptor, bytes.baseAddress, bytes.count)
      }
      if count == 0 { return data }
      if count < 0 {
        if errno == EINTR { continue }
        throw Failure.unreadable
      }
      guard count <= maximumBytes - data.count else { throw Failure.tooLarge }
      data.append(contentsOf: buffer[..<count])
    }
    #else
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var data = Data()
    data.reserveCapacity(min(maximumBytes, 64 * 1024))
    while true {
      let remaining = maximumBytes - data.count
      let chunk = try handle.read(upToCount: min(64 * 1024, remaining + 1)) ?? Data()
      if chunk.isEmpty { return data }
      guard chunk.count <= remaining else { throw Failure.tooLarge }
      data.append(chunk)
    }
    #endif
  }
}
