import SwiftUI

/// 页面持有代次和任务句柄；此处统一单次操作的忙碌状态与过期结果处理。
@MainActor enum ViewRequestTask {
  /// 页面可见时立即刷新，并在取消前按固定间隔继续刷新。
  static func poll(every interval: Duration, refresh: @MainActor () -> Void) async {
    while !Task.isCancelled {
      refresh()
      do { try await Task.sleep(for: interval) } catch { return }
    }
  }

  /// 不占用页面忙碌状态的刷新任务；页面继续负责读取结果和错误提示。
  static func startLoad(request: Binding<ViewRequestGeneration>, pending: Binding<Task<Void, Never>?>,
                        action: @escaping @MainActor (UUID) async -> Void) {
    guard request.wrappedValue.isVisible else { return }
    let id = request.wrappedValue.begin()
    pending.wrappedValue?.cancel()
    pending.wrappedValue = Task {
      defer { if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil } }
      await action(id)
    }
  }

  /// 自行管理忙碌状态的页面用这两个入口替换旧任务并只清理当前任务。
  static func claim(request: Binding<ViewRequestGeneration>, pending: Binding<Task<Void, Never>?>) -> UUID {
    pending.wrappedValue?.cancel()
    return request.wrappedValue.begin()
  }

  static func clear(request: Binding<ViewRequestGeneration>, pending: Binding<Task<Void, Never>?>, id: UUID) {
    if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil }
  }

  /// 页面离开时先使旧结果失效，再取消并释放任务句柄。
  static func disappear(request: Binding<ViewRequestGeneration>, pending: Binding<Task<Void, Never>?>) {
    request.wrappedValue.disappear()
    pending.wrappedValue?.cancel()
    pending.wrappedValue = nil
  }

  /// 取消不占用页面请求状态的独立操作，并使它的旧回调失效。
  static func cancel(generation: Binding<OperationGeneration>, task: Binding<Task<Void, Never>?>) {
    generation.wrappedValue.cancel(task: &task.wrappedValue)
  }

  /// 取消不带代次的短任务，并清空句柄，避免页面重建后保留已完成任务。
  static func cancel(task: Binding<Task<Void, Never>?>) {
    task.wrappedValue?.cancel()
    task.wrappedValue = nil
  }

  static func run(request: Binding<ViewRequestGeneration>, busy: Binding<Bool>,
                  message: Binding<String?>, pending: Binding<Task<Void, Never>?>,
                  canceling: Binding<Task<Void, Never>?>? = nil,
                  clearMessage: Bool = true,
                  isCancellation: @escaping (Error) -> Bool = { $0 is CancellationError },
                  action: @escaping @MainActor (UUID) async throws -> Void) {
    run(request: request, busy: busy, pending: pending, canceling: canceling,
        prepare: { if clearMessage { message.wrappedValue = nil } },
        onError: { error in
          if !isCancellation(error) { message.wrappedValue = error.localizedDescription }
        }, action: action)
  }

  static func run(request: Binding<ViewRequestGeneration>, busy: Binding<Bool>,
                  pending: Binding<Task<Void, Never>?>,
                  canceling: Binding<Task<Void, Never>?>? = nil,
                  prepare: @MainActor () -> Void,
                  onError: @escaping @MainActor (Error) -> Void = { _ in },
                  action: @escaping @MainActor (UUID) async throws -> Void) {
    guard request.wrappedValue.isVisible, !busy.wrappedValue else { return }
    let id = request.wrappedValue.begin()
    canceling?.wrappedValue?.cancel()
    canceling?.wrappedValue = nil
    busy.wrappedValue = true
    prepare()
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
        if request.wrappedValue.isCurrent(id) { onError(error) }
      }
    }
  }
}

/// 页面出现时开启请求代次，离开时使旧结果失效并取消未完成任务；页面自己的离开清理紧跟在公共清理之后。
@MainActor struct ViewRequestLifecycle: ViewModifier {
  @Binding var request: ViewRequestGeneration
  @Binding var pending: Task<Void, Never>?
  let onAppear: @MainActor () -> Void
  let onDisappear: @MainActor () -> Void

  func body(content: Content) -> some View {
    content
      .onAppear {
        request.appear()
        onAppear()
      }
      .onDisappear {
        ViewRequestTask.disappear(request: $request, pending: $pending)
        onDisappear()
      }
  }
}

extension View {
  func trackViewRequest(
    request: Binding<ViewRequestGeneration>,
    pending: Binding<Task<Void, Never>?>,
    onAppear: @escaping @MainActor () -> Void = {},
    onDisappear: @escaping @MainActor () -> Void
  ) -> some View {
    modifier(ViewRequestLifecycle(request: request, pending: pending, onAppear: onAppear, onDisappear: onDisappear))
  }
}
