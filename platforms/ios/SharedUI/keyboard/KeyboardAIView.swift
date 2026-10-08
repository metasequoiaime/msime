import SwiftUI

struct KeyboardDocumentContext: Equatable {
  let document: UUID
  let before: String?
  let selected: String?
  let after: String?

  func matches(document: UUID?, before: String?, selected: String?, after: String?) -> Bool {
    document == self.document && before == self.before && selected == self.selected && after == self.after
  }
}

struct KeyboardAIView: View {
  let text: String
  let configuration: CustomServiceConfiguration
  let canSend: () -> Bool
  let insert: (String) -> Bool
  let close: () -> Void
  @State private var output = ""
  @State private var error = ""
  @State private var errorID = UUID()
  @State private var busy = false
  @State private var operation: Task<Void, Never>?

  var body: some View {
    VStack(spacing: 4) {
      HStack {
        Label("AI 润色", systemImage: "sparkles").font(.headline)
          .dynamicTypeSize(...DynamicTypeSize.xxxLarge).accessibilityAddTraits(.isHeader)
        Spacer()
        Button("关闭", action: close).accessibilityIdentifier("keyboardServiceClose")
      }
      ScrollViewReader { proxy in
        ScrollView {
          VStack(alignment: .leading, spacing: 6) {
            if !error.isEmpty {
              Text(error).foregroundStyle(.red).font(.footnote)
                .id("status").accessibilityIdentifier("keyboardAIStatus")
            }
            Text("发送到 \(destination) · \(configuration.model)")
              .font(.caption).foregroundStyle(.secondary)
            Text(output.isEmpty ? "待发送的选中文字" : "润色结果").font(.caption).foregroundStyle(.secondary)
            Text(output.isEmpty ? text : output).font(.body).frame(maxWidth: .infinity, alignment: .leading)
              .accessibilityIdentifier("keyboardAIText")
          }
        }
        .accessibilityIdentifier("keyboardAIScroll")
        .disablingScrollEdgeEffects()
        .onChange(of: errorID) { _ in proxy.scrollTo("status", anchor: .top) }
      }
      HStack {
        if busy {
          ProgressView()
          Button("取消请求") { operation?.cancel(); busy = false }
        } else if output.isEmpty {
          Button("发送选中文字") { send() }.accessibilityIdentifier("keyboardAISend")
        } else {
          Button("替换选中文字") {
            if insert(output) { close() }
            else { showError("输入位置已变化，请关闭后重新选择文字。") }
          }.accessibilityIdentifier("keyboardAIInsert")
        }
      }.frame(minHeight: 44)
    }
    .padding(.horizontal, 12)
    .buttonStyle(KeyboardPanelButtonStyle())
    .background(Color(uiColor: .secondarySystemBackground))
    .onDisappear { operation?.cancel() }
  }

  private var destination: String {
    // 显示真实的协议：局域网里的 http 接口不能被写成 https，用户要看得出这次请求不加密。
    guard let url = try? configuration.validatedURL(allowsLocalHTTP: true),
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          let scheme = components.scheme?.lowercased(), let host = components.host else { return configuration.endpoint }
    return scheme + "://" + host.lowercased() + (components.port.map { ":\($0)" } ?? "")
  }

  private func showError(_ message: String) {
    error = message
    errorID = UUID()
  }

  private func send() {
    guard canSend(), KeyboardAIService.configuration() == configuration else {
      showError("输入位置或 AI 配置已变化，请关闭后重试。")
      return
    }
    busy = true
    error = ""
    operation = Task { @MainActor in
      do {
        let token = try KeyboardAIService.token(for: configuration)
        let result = try await CustomServiceClient.request(kind: .ai, configuration: configuration, text: text, token: token)
        try Task.checkCancellation()
        guard canSend(), KeyboardAIService.configuration() == configuration else {
          throw ServiceFailure(message: "输入位置或 AI 配置已变化，请关闭后重试。")
        }
        guard !result.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, result.count <= 10_000 else {
          throw ServiceFailure(message: "服务返回的文字为空或超过一万字。")
        }
        output = result
      } catch {
        if !Task.isCancelled { showError(error.localizedDescription) }
      }
      if !Task.isCancelled { busy = false }
    }
  }
}
