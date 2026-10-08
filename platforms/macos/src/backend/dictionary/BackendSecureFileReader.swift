import Foundation
#if canImport(Darwin)
import Darwin
#endif

enum MacSecureFileReader {
  static func readData(from url: URL, maximumBytes: Int) throws -> Data {
    #if canImport(Darwin)
    let descriptor = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
    guard descriptor >= 0 else { throw CocoaError(.fileReadNoPermission) }
    var metadata = stat()
    guard fstat(descriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else {
      close(descriptor)
      throw CocoaError(.fileReadNoPermission)
    }
    let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
    #else
    let handle = try FileHandle(forReadingFrom: url)
    #endif
    defer { try? handle.close() }
    var data = Data()
    data.reserveCapacity(min(maximumBytes, 64 * 1024))
    while data.count <= maximumBytes {
      let chunk = try handle.read(upToCount: min(64 * 1024, maximumBytes + 1 - data.count)) ?? Data()
      if chunk.isEmpty { return data }
      data.append(chunk)
    }
    return data
  }
}
