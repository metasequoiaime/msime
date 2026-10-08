import Foundation
import Darwin

/// Reads a file without allowing a replacement or unexpectedly growing file to
/// force an unbounded allocation. The extra byte distinguishes an exact-limit
/// file from one that is too large.
enum BoundedFileReader {
  enum Failure: Error, Equatable {
    case invalidLimit
    case tooLarge
  }

  static func read(from url: URL, maximumBytes: Int) throws -> Data {
    guard maximumBytes > 0 else { throw Failure.invalidLimit }
    let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else {
      throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
    var metadata = stat()
    guard fstat(descriptor, &metadata) == 0, (metadata.st_mode & S_IFMT) == S_IFREG else {
      let code = POSIXErrorCode(rawValue: errno) ?? .EIO
      close(descriptor)
      throw POSIXError(code)
    }
    let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    defer { try? handle.close() }

    var result = Data()
    result.reserveCapacity(min(maximumBytes, 64 * 1024))
    while true {
      let remaining = maximumBytes - result.count
      let chunk = try handle.read(upToCount: min(64 * 1024, remaining + 1)) ?? Data()
      if chunk.isEmpty { return result }
      guard chunk.count <= remaining else { throw Failure.tooLarge }
      result.append(chunk)
    }
  }
}
