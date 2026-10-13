import SwiftUI
import UIKit

// MARK: - 其他平台下载

/// 「其他平台下载」里的一个平台行。`release` 是下载页接受的平台键。
private struct DownloadPlatform: Identifiable {
  let name: String
  let symbol: String
  let meta: String
  let release: String
  var isCurrentDevice = false

  var id: String { name }
}

struct DesktopDownloadView: View {
  private static let downloadPage = URL(string: "https://msime.app/download/")!
  private static let downloadLabel = "msime.app/download"
  /// 每个安装包也都附在共享客户端仓库的 releases 里。
  private static let repository = "msime"
  private static let releases = URL(string: "https://github.com/metasequoiaime/\(repository)/releases")!

  @Environment(\.openURL) private var openURL
  @State private var copied = false

  private static let desktops = [
    DownloadPlatform(name: "HarmonyOS 2in1", symbol: "laptopcomputer", meta: "电脑与平板二合一 · 从源码构建", release: "harmony-pc"),
    DownloadPlatform(name: "Windows", symbol: "pc", meta: "TSF 输入法 · GitHub 发布页下载", release: "windows"),
    DownloadPlatform(name: "macOS", symbol: "laptopcomputer", meta: "InputMethodKit · 自带自动更新", release: "macos"),
    DownloadPlatform(name: "Linux", symbol: "desktopcomputer", meta: "IBus 与 Fcitx5 · DEB、RPM 与 TGZ", release: "linux")
  ]

  private var mobiles: [DownloadPlatform] {
    let pad = UIDevice.current.userInterfaceIdiom == .pad
    return [
      DownloadPlatform(name: "iOS", symbol: "iphone", meta: "TestFlight 测试版", release: "ios", isCurrentDevice: !pad),
      DownloadPlatform(name: "iPadOS", symbol: "ipad.landscape", meta: "与 iPhone 共用同一个 TestFlight", release: "ios", isCurrentDevice: pad),
      DownloadPlatform(name: "Android", symbol: "smartphone", meta: "各版本的 APK 在 GitHub 发布页", release: "android"),
      DownloadPlatform(name: "HarmonyOS", symbol: "smartphone", meta: "从源码构建", release: "harmony")
    ]
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        linkCard
        DesignGroup(title: "电脑", radius: MetasequoiaTheme.tabCardRadius) { rows(Self.desktops) }
        DesignGroup(title: "手机和平板", radius: MetasequoiaTheme.tabCardRadius) { rows(mobiles) }
        Link(destination: Self.releases) {
          Text("在 GitHub 发布页查看所有版本").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.accent)
        }
        .padding(.horizontal, 16)
        .accessibilityIdentifier("desktopReleaseLink")
      }
      .padding(.horizontal, 16).padding(.top, 8).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("其他平台下载").navigationBarTitleDisplayMode(.inline)
  }

  /// 「在电脑上打开」：`accentSoft` 卡片上的下载页地址，带一个复制胶囊按钮。
  private var linkCard: some View {
    HStack(spacing: 14) {
      Image(systemName: "link").font(.system(size: 20, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.onAccent)
        .frame(width: 44, height: 44)
        .background(MetasequoiaTheme.accent, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 2) {
        Text("在电脑上打开").font(.system(size: 16, weight: .semibold)).foregroundStyle(.primary)
        Text(Self.downloadLabel).designSubtext()
      }
      .accessibilityElement(children: .combine)
      Spacer(minLength: 8)
      Button {
        UIPasteboard.general.url = Self.downloadPage
        copied = true
        ToastCenter.shared.show("已复制下载链接")
      } label: {
        DesignPillLabel(title: copied ? "已复制" : "复制链接", style: .filled, size: .page)
      }
      .buttonStyle(.plain)
      .accessibilityLabel(copied ? "已复制下载页链接" : "复制下载页链接")
      .accessibilityIdentifier("copyDesktopDownloadLink")
    }
    .padding(16)
    .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
  }

  @ViewBuilder private func rows(_ platforms: [DownloadPlatform]) -> some View {
    ForEach(Array(platforms.enumerated()), id: \.element.id) { index, platform in
      if index > 0 { DesignDivider(leading: 66) }
      row(platform)
    }
  }

  private func row(_ platform: DownloadPlatform) -> some View {
    HStack(spacing: 14) {
      Image(systemName: platform.symbol).font(.system(size: 17))
        .foregroundStyle(.primary)
        .frame(width: 36, height: 36)
        .background(MetasequoiaTheme.segBg, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 2) {
        Text(platform.name).font(.system(size: 15, weight: .semibold)).foregroundStyle(.primary)
        Text(platform.meta).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
      }
      .accessibilityElement(children: .combine)
      Spacer(minLength: 8)
      if platform.isCurrentDevice {
        DesignPillLabel(title: "当前设备", style: .neutral, size: .page)
      } else {
        Button { openURL(Self.releaseURL(platform.release)) } label: {
          DesignPillLabel(title: "获取", size: .page)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("获取，\(platform.name)")
      }
    }
    .padding(.horizontal, 16).padding(.vertical, 10)
    .frame(maxWidth: .infinity, minHeight: 58, alignment: .leading)
  }

  /// 定位到某个平台分区的下载页，与 Android「其他平台下载」打开的地址相同。
  private static func releaseURL(_ release: String) -> URL {
    var components = URLComponents(url: downloadPage, resolvingAgainstBaseURL: false)!
    components.queryItems = [URLQueryItem(name: "release", value: release)]
    return components.url!
  }
}

// MARK: - 关于

/// 本应用在 App Store 上的条目，从 iTunes lookup API 读取：用于写评价链接的 id、在售版本和商店页面。
private struct AppStoreListing: Sendable {
  let trackID: Int
  let version: String
  let page: URL

  private struct Response: Decodable {
    struct Result: Decodable {
      let trackId: Int
      let version: String
      let trackViewUrl: String
    }
    let results: [Result]
  }

  /// 这个 bundle id 在设备所在商店地区的条目；应用在那里未上架时为 nil，TestFlight 构建就是这样。
  static func fetch() async throws -> AppStoreListing? {
    guard let bundleID = Bundle.main.bundleIdentifier else { return nil }
    var components = URLComponents(string: "https://itunes.apple.com/lookup")!
    var query = [URLQueryItem(name: "bundleId", value: bundleID)]
    if let region = Locale.current.region?.identifier, region.count == 2 {
      query.append(URLQueryItem(name: "country", value: region.lowercased()))
    }
    components.queryItems = query
    var request = URLRequest(url: components.url!, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 15)
    request.httpMethod = "GET"
    let (data, response) = try await URLSession.shared.data(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else { throw URLError(.badServerResponse) }
    guard let result = try JSONDecoder().decode(Response.self, from: data).results.first,
          let page = URL(string: result.trackViewUrl), page.scheme == "https",
          let host = page.host, host == "apps.apple.com" || host == "itunes.apple.com" else { return nil }
    return AppStoreListing(trackID: result.trackId, version: result.version, page: page)
  }

  var reviewURL: URL? { URL(string: "itms-apps://itunes.apple.com/app/id\(trackID)?action=write-review") }
}

struct AboutView: View {
  private enum UpdateState: Equatable {
    case idle, checking, latest
    case available(URL)
  }

  @Environment(\.openURL) private var openURL
  @State private var update = UpdateState.idle
  @State private var listing: AppStoreListing?
  @State private var requestState = AboutUpdateRequestState()
  @State private var updateTask: Task<Void, Never>?
  /// lookup 返回商店在本地区没有这个应用的条目，TestFlight 构建就是这样，它的更新走 TestFlight。此时「检查更新」没有可比较的对象，所以和「给我们评分」一样隐藏。
  @State private var unlisted = false
  private let noticeCount = LicenseNotice.bundled.count

  private var version: String {
    let info = Bundle.main.infoDictionary ?? [:]
    let short = info["CFBundleShortVersionString"] as? String ?? "—"
    let build = info["CFBundleVersion"] as? String ?? "—"
    let system = UIDevice.current.userInterfaceIdiom == .pad ? "iPadOS" : "iOS"
    return "版本 \(short)（\(build)）· \(system)"
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 24) {
        hero
        DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
          Link(destination: URL(string: "https://msime.app/")!) {
            SettingsNavigationLabel(title: "官网", value: "msime.app")
          }
          .buttonStyle(PressFillButtonStyle())
          DesignDivider()
          Link(destination: URL(string: "https://msime.app/privacy/")!) {
              SettingsNavigationLabel(title: "隐私政策")
          }
          .buttonStyle(PressFillButtonStyle())
          if noticeCount > 0 {
            DesignDivider()
            NavigationLink(destination: LicensesView()) {
              SettingsNavigationLabel(title: "开源许可", value: "\(noticeCount) 个组件")
            }
            .buttonStyle(PressFillButtonStyle())
            .accessibilityIdentifier("aboutLicensesLink")
          }
          DesignDivider()
          Link(destination: URL(string: "https://github.com/metasequoiaime/msime")!) {
              SettingsNavigationLabel(title: "源代码", value: "GitHub")
          }
          .buttonStyle(PressFillButtonStyle())
          if let review = listing?.reviewURL {
            DesignDivider()
            Link(destination: review) {
              SettingsNavigationLabel(title: "给我们评分")
            }
            .buttonStyle(PressFillButtonStyle())
          }
        }
        Text("© 2026 Metasequoia · 输入内容默认只在本机处理")
          .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
          .multilineTextAlignment(.center)
      }
      .padding(.horizontal, 16).padding(.top, 16).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("关于").navigationBarTitleDisplayMode(.inline)
    .task {
      // 「给我们评分」要用的商店 id，以及到底有没有条目；lookup 失败时只是不显示这一行，并保留「检查更新」以便重试。
      guard listing == nil else { return }
      let id = requestState.currentID
      do {
        let found = try await AppStoreListing.fetch()
        guard !Task.isCancelled, requestState.isCurrent(id) else { return }
        listing = found
        unlisted = found == nil
      } catch {
        // lookup 本身失败（离线、服务器出错）：对条目还一无所知。
        return
      }
    }
    .onDisappear {
      requestState.invalidate()
      ViewRequestTask.cancel(task: $updateTask)
      if update == .checking { update = .idle }
    }
  }

  private var hero: some View {
    VStack(spacing: 0) {
      AppMarkDisc(diameter: 116, markSize: 64)
      Text("水杉输入法").font(.system(size: 22, weight: .bold)).padding(.top, 16)
      Text(version).designSubtext()
        .padding(.top, 6)
        .accessibilityIdentifier("aboutAppVersion")
      if !unlisted {
        Button(action: checkForUpdate) { updatePill }
          .buttonStyle(.plain)
          .disabled(update == .checking || update == .latest)
          .padding(.top, 14)
          .accessibilityIdentifier("aboutCheckUpdate")
      }
    }
    .frame(maxWidth: .infinity)
    .padding(.top, 8)
  }

  @ViewBuilder private var updatePill: some View {
    switch update {
    case .idle: DesignPillLabel(title: "检查更新", style: .filled, size: .page)
    case .checking: DesignPillLabel(title: "正在检查…", style: .filled, size: .page).opacity(0.7)
    case .latest:
      Text("✓ 已是最新版本").font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
        .padding(.vertical, 6)
    case .available: DesignPillLabel(title: "有新版本 ›", style: .filled, size: .page)
    }
  }

  private func checkForUpdate() {
    if case .available(let page) = update {
      openURL(page)
      return
    }
    guard update == .idle, let id = requestState.begin() else { return }
    update = .checking
    updateTask = Task { @MainActor in
      defer { if requestState.isCurrent(id) { updateTask = nil } }
      do {
        let found = try await AppStoreListing.fetch()
        guard !Task.isCancelled, requestState.finish(id) else { return }
        guard let found else {
          // 这不是失败：商店里没有可比较的条目，所以胶囊按钮直接消失，而不是每次点按都失败。
          update = .idle
          unlisted = true
          return
        }
        listing = found
        let installed = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
        update = found.version.compare(installed, options: .numeric) == .orderedDescending ? .available(found.page) : .latest
      } catch {
        guard !Task.isCancelled, requestState.finish(id) else { return }
        update = .idle
        ToastCenter.shared.show("暂时无法检查更新")
      }
    }
  }
}

// MARK: - 开源许可

/// 应用打包的许可证或声明文件（见 `project.yml` 中 `MSIMEApp` target 的 resources）。
private struct LicenseNotice: Identifiable {
  let file: String
  let title: String
  let detail: String

  var id: String { file }
  var url: URL? { Bundle.main.url(forResource: file, withExtension: "txt") }

  private static let all = [
    LicenseNotice(file: "MLKit-NOTICES", title: "Google ML Kit", detail: "手写识别 · 第三方声明"),
    LicenseNotice(file: "MLKit-Dependencies", title: "ML Kit 依赖组件", detail: "手写识别 · 依赖清单"),
    LicenseNotice(file: "GoogleSignIn-Dependencies", title: "Google Sign-In", detail: "Google 登录 · Apache-2.0"),
    LicenseNotice(file: "VoiceRuntime-NOTICES", title: "sherpa-onnx", detail: "语音识别 · Apache-2.0 与 MIT"),
    LicenseNotice(file: "onnxruntime-ThirdPartyNotices", title: "ONNX Runtime", detail: "语音识别 · 第三方声明"),
    LicenseNotice(file: "libhangul-hanja-BSD-3-Clause", title: "libhangul 汉字表", detail: "韩文 · BSD-3-Clause"),
    LicenseNotice(file: "rime-cantonese-CC-BY-4.0", title: "rime-cantonese", detail: "粤语 · CC BY 4.0"),
    LicenseNotice(file: "libchewing-data-LGPL-2.1", title: "libchewing-data", detail: "注音 · LGPL-2.1"),
    LicenseNotice(file: "rime-stroke-LGPL-3.0", title: "rime-stroke", detail: "笔画 · LGPL-3.0"),
    LicenseNotice(file: "vi-MIT", title: "vi", detail: "越南文 · MIT"),
    LicenseNotice(file: "ewts-MIT", title: "ewts", detail: "藏文 · MIT"),
    LicenseNotice(file: "kfcd-chaizi-CC-BY-3.0", title: "開放詞典 拆字表", detail: "拆字 · CC BY 3.0"),
    LicenseNotice(file: "Unicode-License-V3", title: "Unicode 汉字数据库", detail: "拆字读音 · Unicode License V3")
  ]

  /// 本次构建的 bundle 里实际存在的声明文件。
  static let bundled = all.filter { $0.url != nil }
}

struct LicensesView: View {
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 7) {
        DesignCard(radius: MetasequoiaTheme.tabCardRadius) {
          ForEach(Array(LicenseNotice.bundled.enumerated()), id: \.element.id) { index, notice in
            if index > 0 { DesignDivider() }
            NavigationLink(destination: LicenseTextView(notice: notice)) {
              HStack(spacing: 8) {
                VStack(alignment: .leading, spacing: 2) {
                  Text(notice.title).font(.system(size: 16)).foregroundStyle(.primary)
                  Text(notice.detail).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
                }
                Spacer(minLength: 8)
                DesignChevron()
              }
              .padding(.horizontal, 16).padding(.vertical, 8)
              .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
              .contentShape(Rectangle())
              .accessibilityElement(children: .combine)
            }
            .buttonStyle(PressFillButtonStyle())
          }
        }
        Text("水杉输入法本身以开源许可发布，源代码见关于页的 GitHub。")
          .designSubtext()
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 16)
      }
      .padding(.horizontal, 16).padding(.top, 16).padding(.bottom, 28)
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
    }
    .designCanvasBackground()
    .navigationTitle("开源许可").navigationBarTitleDisplayMode(.inline)
  }
}

/// 一份声明的全文。有些声明超过 1 MB，所以在主线程之外读取文件，并用 `UITextView` 显示，它对这么长的文本是惰性排版的。
private struct LicenseTextView: View {
  let notice: LicenseNotice
  @State private var text: String?

  var body: some View {
    Group {
      if let text {
        NoticeTextView(text: text)
      } else {
        ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
    .designCanvasBackground()
    .navigationTitle(notice.title).navigationBarTitleDisplayMode(.inline)
    .task {
      guard text == nil, let url = notice.url else { return }
      text = await Task.detached(priority: .userInitiated) {
        (try? String(contentsOf: url, encoding: .utf8)) ?? (try? String(contentsOf: url, encoding: .isoLatin1)) ?? ""
      }.value
    }
  }
}

private struct NoticeTextView: UIViewRepresentable {
  let text: String

  func makeUIView(context: Context) -> UITextView {
    let view = UITextView()
    view.isEditable = false
    view.isSelectable = true
    view.backgroundColor = .clear
    view.adjustsFontForContentSizeCategory = true
    view.font = .monospacedSystemFont(ofSize: UIFont.preferredFont(forTextStyle: .footnote).pointSize, weight: .regular)
    view.textColor = .label
    view.textContainerInset = UIEdgeInsets(top: 16, left: 12, bottom: 28, right: 12)
    view.text = text
    return view
  }

  func updateUIView(_ view: UITextView, context: Context) {
    if view.text != text { view.text = text }
  }
}
