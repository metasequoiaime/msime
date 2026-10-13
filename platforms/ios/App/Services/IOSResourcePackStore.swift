import Combine
import Foundation
import Network

/// 下载状态只由主应用维护，键盘扩展不负责联网安装。
@MainActor
final class IOSResourcePackStore: ObservableObject {
  static let shared = IOSResourcePackStore()
  struct Status {
    var phase: String
    var percent: Int = 0
    var message: String {
      switch phase {
      case "download": return "下载中 \(percent)%"
      case "verify": return "校验中"
      case "done": return "安装完成"
      case "cancelled": return "已取消，可继续下载"
      case "failed": return "下载失败，点按重试"
      default: return "准备下载"
      }
    }
    var running: Bool { !["failed", "cancelled"].contains(phase) }
  }

  @Published private(set) var packs: [String: IOSResourcePacks.Pack] = [:]
  @Published private(set) var statuses: [String: Status] = [:]
  @Published var meteredRequest: String?
  @Published private(set) var loadFailed = false
  private var expensiveNetwork = false
  private let monitor = NWPathMonitor()

  init() {
    monitor.pathUpdateHandler = { [weak self] path in
      Task { @MainActor in self?.expensiveNetwork = path.isExpensive || path.isConstrained }
    }
    monitor.start(queue: DispatchQueue(label: "app.msime.ios.resource-network"))
  }

  func reload() {
    Task {
      let result = await Task.detached { Result { try IOSResourcePacks.list() } }.value
      switch result {
      case .success(let listed):
        packs = Dictionary(uniqueKeysWithValues: listed.map { ($0.id, $0) })
        InputSchemePreference.refreshResourceAvailability()
        loadFailed = false
      case .failure: loadFailed = true
      }
    }
  }

  func available(_ id: String) -> Bool { IOSResourcePacks.bundled(id) || packs[id]?.installed == true }
  func size(_ id: String) -> String {
    guard let size = packs[id]?.size else { return "获取大小中" }
    return ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file)
  }
  func describe(_ id: String) -> String {
    if let status = statuses[id] { return status.message }
    if available(id) { return "已下载" }
    return "需下载 \(size(id))"
  }

  func request(_ id: String) {
    guard !available(id), statuses[id]?.running != true else { return }
    if expensiveNetwork { meteredRequest = id }
    else { start(id) }
  }

  func start(_ id: String) {
    meteredRequest = nil
    guard !available(id), statuses[id]?.running != true else { return }
    statuses[id] = Status(phase: "download")
    Task {
      let result = await Task.detached {
        Result {
          try IOSResourcePacks.install(id) { [weak self] phase, done, total in
            let percentage = total == 0 ? 0 : Int(min(100, Double(done) / Double(total) * 100))
            Task { @MainActor in
              guard let self, self.statuses[id]?.running == true else { return }
              if self.statuses[id]?.percent != percentage || self.statuses[id]?.phase != phase {
                self.statuses[id] = Status(phase: phase, percent: percentage)
              }
            }
          }
        }
      }.value
      switch result {
      case .success:
        statuses[id] = nil
        reload()
        ToastCenter.shared.show("词库已下载，重新打开键盘即可使用")
      case .failure(let error):
        let cancelled = (error as? IOSResourcePacks.Failure)?.code == "local_model_cancelled"
        statuses[id] = Status(phase: cancelled ? "cancelled" : "failed")
      }
    }
  }

  func cancel(_ id: String) { IOSResourcePacks.cancel(id) }
}
