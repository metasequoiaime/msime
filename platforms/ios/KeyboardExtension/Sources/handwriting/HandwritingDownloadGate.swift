struct HandwritingDownloadGate {
  private(set) var generation: UInt64 = 0
  private(set) var active = false

  mutating func setActive(_ value: Bool) {
    guard active != value else { return }
    active = value
    generation &+= 1
  }

  mutating func beginDownload() -> UInt64 {
    generation &+= 1
    return generation
  }

  func accepts(_ token: UInt64) -> Bool {
    active && generation == token
  }
}
