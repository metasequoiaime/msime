import SwiftUI

/// 「我的」→「同步」→「绑定 QQ 群助手」，对应 Android 的 `QqLinksPage` 和共享设置页的 `QqLinkCard`：输入机器人给的 8 位绑定码，列出已有的绑定，可以解绑。绑定后在 QQ 群里 @机器人 发 /stats 能看到打字数据，数据来自统计页的「上传到云端」。
///
/// 经 Rust 的 `msime_client_account`（`qq_links`、`qq_link_redeem`、`qq_link_delete`，#8201 迁移计划 4c），只用登录账号，匿名账号不绑定；没登录时说明要先登录。各种失败的说法与共享设置页、Android 相同（`QQLinksService.message`）。
struct QQLinksView: View {
  private enum Load: Equatable {
    case loading
    case signedOut
    case failed(String)
    case loaded([QQLinksService.Link])
  }

  @State private var load = Load.loading
  @State private var code = ""
  @State private var busy = false
  @State private var removing: QQLinksService.Link?

  private static let intro = "在水杉输入法的 QQ 群里 @机器人 发 /bind，把收到的 8 位绑定码填在这里。绑定后发 /stats 能看到你的打字数据（先在「统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。"

  var body: some View {
    Form {
      Section {
        Text(Self.intro).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
      }
      if load == .signedOut {
        Section {
          Text("登录水杉账号后才能绑定。").accessibilityIdentifier("qqLinksSignedOut")
        }
      } else {
        Section {
          TextField("8 位数字", text: $code)
            .keyboardType(.numberPad)
            .textContentType(.oneTimeCode)
            .accessibilityIdentifier("qqLinkCode")
          Button("绑定") { redeem() }
            .disabled(busy || QQLinksService.normalizedCode(code) == nil)
            .accessibilityIdentifier("qqLinkRedeem")
        } header: {
          DesignSectionHeader(info: "10 分钟内有效，只能用一次") { Text("绑定码") }
        }
        Section {
          links
        } header: {
          Text("已绑定")
        } footer: {
          Text("解绑后，那个群里的 /stats 不再显示你的数据；在群里发 /unbind 效果相同。")
        }
      }
    }
    .navigationTitle("绑定 QQ 群助手").navigationBarTitleDisplayMode(.inline)
    .task { await reload() }
    .confirmationDialog(removing.map { "解绑「绑定于 \(Self.time($0.createdAt))」？" } ?? "",
                        isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                        titleVisibility: .visible) {
      Button("解绑", role: .destructive) {
        if let link = removing { remove(link) }
      }
      Button("取消", role: .cancel) {}
    }
  }

  @ViewBuilder private var links: some View {
    switch load {
    case .loading, .signedOut:
      HStack(spacing: 8) {
        ProgressView()
        Text("正在读取…").foregroundStyle(MetasequoiaTheme.sub)
      }
    case .failed(let message):
      Text(message).foregroundStyle(MetasequoiaTheme.sub)
    case .loaded(let links) where links.isEmpty:
      Text("还没有绑定。").foregroundStyle(MetasequoiaTheme.sub)
    case .loaded(let links):
      ForEach(links) { link in
        HStack {
          Text("绑定于 " + Self.time(link.createdAt))
          Spacer(minLength: 8)
          Button("解绑", role: .destructive) { removing = link }
            .buttonStyle(.borderless)
            .disabled(busy)
        }
        .accessibilityIdentifier("qqLink_\(link.id)")
      }
    }
  }

  @MainActor private func reload() async {
    do {
      load = .loaded(try await QQLinksService.links())
    } catch QQLinksService.Failure.signedOut {
      load = .signedOut
    } catch {
      if AccountRequestPolicy.isCancellation(error) { return }
      load = .failed(QQLinksService.message(error))
    }
  }

  private func redeem() {
    let entered = code
    busy = true
    Task { @MainActor in
      defer { busy = false }
      do {
        try await QQLinksService.redeem(entered)
        code = ""
        ToastCenter.shared.show("已绑定。在 QQ 群里 @机器人 发 /stats 就能看到你的打字数据。")
        await reload()
      } catch QQLinksService.Failure.signedOut {
        load = .signedOut
      } catch {
        ToastCenter.shared.show(QQLinksService.message(error, redeeming: true))
      }
    }
  }

  private func remove(_ link: QQLinksService.Link) {
    removing = nil
    busy = true
    Task { @MainActor in
      defer { busy = false }
      do {
        try await QQLinksService.delete(link.id)
        ToastCenter.shared.show("已解绑。")
        await reload()
      } catch QQLinksService.Failure.signedOut {
        load = .signedOut
      } catch {
        ToastCenter.shared.show(QQLinksService.message(error))
      }
    }
  }

  /// 绑定时间按本机时区显示成「2026-10-17 12:34」，与 Android 相同；读不懂时原样显示。
  static func time(_ createdAt: String) -> String {
    let parser = ISO8601DateFormatter()
    parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let plain = ISO8601DateFormatter()
    plain.formatOptions = [.withInternetDateTime]
    guard let date = parser.date(from: createdAt) ?? plain.date(from: createdAt) else { return createdAt }
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "yyyy-MM-dd HH:mm"
    return formatter.string(from: date)
  }
}
