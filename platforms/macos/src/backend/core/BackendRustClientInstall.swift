import CMSIMEClient
import Foundation

/// 由 `input_method_main.mm` 在 `dlopen` 后用 `dlsym` 找到并调用，所以必须是导出的全局符号。输入法可执行文件链着 Rust 宿主库，把 `msime_rust_client_linked()` 填好的表交过来；表的版本或大小不对时不收、返回 false，之后每个 Rust 入口都报 `rust_client_unavailable`。
@_cdecl("MSIMEInstallRustClient")
public func msimeInstallRustClient(_ table: UnsafeRawPointer?) -> Bool {
  // 先只读开头的 version 和 size：调用方的表可能来自别的版本、比本版本的结构体短，按本版本的大小整个读出来会越界。
  guard let table, table.load(as: UInt32.self) == MSIME_RUST_CLIENT_VERSION,
        Int(table.load(fromByteOffset: MemoryLayout<UInt32>.size, as: UInt32.self)) == MemoryLayout<msime_rust_client>.size else {
    return false
  }
  return BackendRustClient.install(table.load(as: msime_rust_client.self))
}
