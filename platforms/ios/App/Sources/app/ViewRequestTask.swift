import SwiftUI

/// 页面持有代次和任务句柄；此处统一单次操作的忙碌状态与过期结果处理。
@MainActor enum ViewRequestTask {
  static func run(request: Binding<ViewRequestGeneration>, busy: Binding<Bool>,
                  message: Binding<String?>, pending: Binding<Task<Void, Never>?>,
                  canceling: Binding<Task<Void, Never>?>? = nil,
                  clearMessage: Bool = true,
                  isCancellation: @escaping (Error) -> Bool = { $0 is CancellationError },
                  action: @escaping @MainActor (UUID) async throws -> Void) {
    guard request.wrappedValue.isVisible, !busy.wrappedValue else { return }
    let id = request.wrappedValue.begin()
    canceling?.wrappedValue?.cancel()
    canceling?.wrappedValue = nil
    busy.wrappedValue = true
    if clearMessage { message.wrappedValue = nil }
    pending.wrappedValue = Task {
      defer {
        if request.wrappedValue.isCurrent(id) {
          busy.wrappedValue = false
          pending.wrappedValue = nil
        }
      }
      do {
        try await action(id)
        try request.wrappedValue.checkCurrent(id)
      }
      catch is CancellationError { }
      catch {
        if request.wrappedValue.isCurrent(id), !isCancellation(error) {
          message.wrappedValue = error.localizedDescription
        }
      }
    }
  }
}
