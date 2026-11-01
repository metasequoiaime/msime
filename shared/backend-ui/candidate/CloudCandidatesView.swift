import SwiftUI

/// 「云端候选与排序」：查询、调频、固定位置和删除候选都经 Rust 的 `msime_client_cloud_dictionary`（`candidates`、`fixed_positions`、`rank`、`set_fixed_position`、`remove_candidate`，#8201 迁移计划 4c）。macOS 与 iOS 共用；页面只按打开时的 user.id 绑定（C 上拿不到会话代次），每个请求前后都核对，退出或换了号就丢掉结果。
struct CloudCandidatesView: View {
  let kind: BackendAccountClient.DictionaryKind
  let accountID: String
  /// nil 是宿主没有注册会话存储，每个操作都报 `rust_client_unavailable`。
  let rust: BackendRustAccount?
  @State private var text = ""
  @State private var scheme = "pinyin"
  @State private var profile = "xiaohe"
  @State private var jianpin = false
  @State private var mode = BackendAccountClient.RankingMode.pin
  @State private var step = 1
  @State private var trigger = 1
  @State private var forceTop = false
  @State private var position = 1
  @State private var page: BackendAccountClient.PersonalCandidates?
  @State private var query: BackendAccountClient.CandidateQuery?
  @State private var positions: [BackendAccountClient.FixedPosition] = []
  @State private var action: Action?
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  private enum Action: Sendable {
    case rank(BackendAccountClient.PersonalCandidate)
    case fix(BackendAccountClient.PersonalCandidate)
    case remove(BackendAccountClient.PersonalCandidate)
    case unfix(BackendAccountClient.FixedPosition)
    var title: String {
      switch self { case .rank: return "调整云端候选排序？"; case .fix: return "固定云端候选位置？"; case .remove: return "删除云端候选？"; case .unfix: return "取消固定位置？" }
    }
  }

  init(kind: BackendAccountClient.DictionaryKind, accountID: String, rust: BackendRustAccount? = .shared) {
    self.kind = kind; self.accountID = accountID; self.rust = rust
  }

  var body: some View {
    List {
      Section {
        TextField("输入编码", text: $text).backendCodeInput()
        if kind == .pinyin {
          Toggle("简拼候选", isOn: $jianpin)
          Picker("编码方案", selection: $scheme) { Text("全拼").tag("pinyin"); Text("双拼").tag("shuangpin") }
          if scheme == "shuangpin" {
            Picker("双拼方案", selection: $profile) {
              Text("小鹤").tag("xiaohe"); Text("自然码").tag("ziranma")
              Text("微软").tag("microsoft"); Text("首道").tag("shoudao")
            }
          }
        }
        SettingsActionRow(title: "查询云端候选", symbol: "magnifyingglass", enabled: !text.isEmpty) {
          run { try await load(.init(text: text, kind: jianpin && kind == .pinyin ? "jianpin" : kind.rawValue, scheme: scheme, profile: profile, limit: 100)) }
        }
      } header: {
        SettingsSectionInfoHeader(subject: kind.title, info: "仅在点击查询时发送这里输入的编码。排序和固定位置由服务器输入引擎计算，修改保存到当前账号；本机键盘尚需完整同步才能采用云端状态。") { Text(kind.title) }
      }
      if kind != .quick {
        Section("调频方式") {
          Picker("方式", selection: $mode) { ForEach(BackendAccountClient.RankingMode.allCases) { Text($0.title).tag($0) } }
          Stepper("线性步长：\(step)", value: $step, in: 1...100).disabled(mode != .linear)
          Stepper("触发次数：\(trigger)", value: $trigger, in: 1...10)
          Toggle("本次强制置顶", isOn: $forceTop)
          Stepper("固定到第 \(position) 位", value: $position, in: 1...5)
        }
      }
      if let page {
        Section {
          if page.candidates.isEmpty { Text("没有匹配的候选。").foregroundStyle(.secondary) }

          ForEach(Array(page.candidates.enumerated()), id: \.element.id) { index, candidate in
            VStack(alignment: .leading, spacing: 3) {
              Text("\(index + 1). \(candidate.word)")
              Text("\(candidate.code) · 权重 \(candidate.weight)").font(.caption).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .onTapGesture { if kind != .quick { action = .rank(candidate) } }
            .swipeActions(edge: .trailing) {
              if kind != .quick {
                Button("删除", role: .destructive) { action = .remove(candidate) }
                  .disabled(kind != .english && candidate.word.unicodeScalars.count <= 1)
                Button("固定") { action = .fix(candidate) }.tint(.accentColor)
              }
            }
          }
        } header: {
          SettingsSectionInfoHeader(subject: "当前查询候选 · 版本 \(page.revision)",
                                    info: kind != .quick ? "点一条调频，左滑固定到指定位置或删除。" : nil) {
            Text("当前查询候选 · 版本 \(page.revision)")
          }
        }
        if !positions.isEmpty {
          Section {
            ForEach(positions) { item in
              Text("第 \(item.position) 位 · \(item.word)")
                .swipeActions(edge: .trailing) {
                  Button("取消固定", role: .destructive) { action = .unfix(item) }
                }
            }
          } header: {
            SettingsSectionInfoHeader(subject: "此查询的固定位置", info: "左滑取消固定。") { Text("此查询的固定位置") }
          }
        }
      }
    }
    .settingsStatus(busy: busy, message: message)
    .navigationTitle("云端候选排序")
    .disabled(busy)
    .onDisappear { pending?.cancel(); page = nil; positions = [] }
    .alert(action?.title ?? "确认操作", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } })) {
      Button("取消", role: .cancel) { action = nil }
      Button("确认") {
        guard let selected = action, let page, let query else { return }
        run { try await apply(selected, page: page, query: query) }
        action = nil
      }
    } message: { Text("修改当前查询的云端状态；固定位置会替换该位置原有的词条。若其他设备已更新，需重新查询再确认。") }
  }
  @MainActor private func load(_ query: BackendAccountClient.CandidateQuery) async throws {
    let result = try BackendRustAccount.decode(BackendAccountClient.PersonalCandidates.self,
                                               try await request(Self.queryAction("candidates", query)))
    let fixed: [BackendAccountClient.FixedPosition]
    if query.kind != "quick" && !result.context.isEmpty {
      fixed = try BackendRustAccount.decode(BackendAccountClient.FixedPositions.self,
        try await request(["operation": "fixed_positions", "context": result.context, "offset": 0])).positions
    } else { fixed = [] }
    self.query = query; page = result; positions = fixed
  }
  @MainActor private func apply(_ action: Action, page: BackendAccountClient.PersonalCandidates, query: BackendAccountClient.CandidateQuery) async throws {
    let unchangedStatus = "云端状态已更新，本机设置保持原样。"
    let status: String
    switch action {
    case .rank(let candidate):
      var rank = Self.queryAction("rank", query)
      rank["code"] = candidate.mutationCode; rank["word"] = candidate.word; rank["revision"] = page.revision
      rank["mode"] = mode.rawValue; rank["linear_step"] = step; rank["trigger_count"] = trigger; rank["force_top"] = forceTop
      let result = try BackendRustAccount.object(try await request(rank))
      guard let changed = result["changed"] as? Bool, let count = (result["selection_count"] as? NSNumber)?.intValue else {
        throw BackendRustClient.Failure.invalidResponse
      }
      status = changed ? "云端排序已更新。" : "已记录本次选择，本次未调整排序；累计 \(count) 次。"
    case .fix(let candidate):
      _ = try await request(["operation": "set_fixed_position", "context": page.context, "code": candidate.mutationCode,
                             "word": candidate.word, "position": position, "revision": page.revision])
      status = unchangedStatus
    case .unfix(let item):
      _ = try await request(["operation": "set_fixed_position", "context": item.context, "code": item.code,
                             "word": item.word, "position": NSNull(), "revision": page.revision])
      status = unchangedStatus
    case .remove(let candidate):
      var remove = Self.queryAction("remove_candidate", query)
      remove["code"] = candidate.mutationCode; remove["word"] = candidate.word; remove["revision"] = page.revision
      _ = try await request(remove)
      status = unchangedStatus
    }
    try await load(query)
    message = status
  }
  /// 带查询的 action：`candidates`、`rank`、`remove_candidate` 都带同样的五项。
  static func queryAction(_ operation: String, _ query: BackendAccountClient.CandidateQuery) -> [String: Any] {
    ["operation": operation, "text": query.text, "kind": query.kind, "scheme": query.scheme,
     "profile": query.profile, "limit": query.limit]
  }
  /// 发一个云词库 action；请求前后都核对打开页面时的账号。
  @MainActor private func request(_ action: [String: Any]) async throws -> Data {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    guard try await rust.status()?.id == accountID else { throw CancellationError() }
    let data = try await rust.cloudDictionary(action)
    guard try await rust.status()?.id == accountID else { throw CancellationError() }
    try Task.checkCancellation()
    return data
  }
  @MainActor private func run(_ work: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await work() }
      catch is CancellationError { page = nil; positions = []; query = nil }
      catch { message = error.localizedDescription }
    }
  }
}
