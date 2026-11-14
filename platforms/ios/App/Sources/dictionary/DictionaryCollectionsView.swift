import SwiftUI
import UniformTypeIdentifiers

/// 分组管理只持有选择和表单状态，导入去重、容量、启停和删除归属都由共享层执行。
struct DictionaryCollectionsView: View {
  @State private var document: DictionaryCollectionsBridge.Document?
  @State private var selection: Set<String> = []
  @State private var busy = false
  @State private var message: String?
  @State private var confirmsDelete = false
  @State private var naming = false
  @State private var name = ""
  @State private var renaming: String?
  @State private var choosingFile = false
  @State private var importFile: URL?
  @State private var importName = ""
  @State private var importKind = "pinyin"
  @State private var importFormat = "standard"
  @State private var importTarget = ""
  @State private var importSheet = false

  var body: some View {
    List {
      if let document {
        ForEach(["user", "import", "community"], id: \.self) { source in
          let items = document.collections.filter { $0.source.type == source }
          if !items.isEmpty {
            Section {
              ForEach(items) { item in
                HStack(spacing: 12) {
                  Button {
                    if !selection.insert(item.id).inserted { selection.remove(item.id) }
                  } label: {
                    Image(systemName: selection.contains(item.id) ? "checkmark.circle.fill" : "circle")
                      .font(.title3)
                  }
                  .buttonStyle(.plain)
                  .accessibilityLabel("选择「\(item.name)」")
                  .accessibilityValue(selection.contains(item.id) ? "已选" : "未选")
                  VStack(alignment: .leading, spacing: 4) {
                    Text(item.name)
                    Text("\(item.entry_count) 条 · \(item.enabled ? "已启用" : "已停用")")
                      .font(.caption).foregroundStyle(.secondary)
                    if item.pending > 0 { Text("\(item.pending) 条待写入").font(.caption).foregroundStyle(.secondary) }
                  }
                  Spacer()
                  Menu {
                    Button("改名") { renaming = item.id; name = item.name; naming = true }
                    Button(item.enabled ? "停用" : "启用") {
                      perform(["operation": "set_enabled", "id": item.id, "enabled": !item.enabled])
                    }
                  } label: { Image(systemName: "ellipsis.circle") }
                  .accessibilityLabel("「\(item.name)」的操作")
                }
              }
            } header: {
              SettingsGroupHeader(title: source == "user" ? "自建词库" : source == "import" ? "文件导入" : "社区词库")
            }
          }
        }
        if document.collections.isEmpty { Text("还没有分组").foregroundStyle(.secondary) }
        if !selection.isEmpty {
          Section {
            Button("启用所选（\(selection.count)）") { changeSelected(enabled: true) }
            Button("停用所选（\(selection.count)）") { changeSelected(enabled: false) }
            Button("删除所选（\(selection.count)）", role: .destructive) { confirmsDelete = true }
          }
        }
      }
      if busy { ProgressView("正在处理…") }
    }
    .designPage()
    .navigationTitle("词库分组")
    .toolbar {
      ToolbarItem(placement: .topBarTrailing) {
        Menu {
          Button("新建分组") { renaming = nil; name = ""; naming = true }
          Button("导入词库") { choosingFile = true }
          Button("全选") { selection = Set(document?.collections.map(\.id) ?? []) }
          Button("取消选择") { selection.removeAll() }
          Button("刷新") { perform(["operation": "load"]) }
        } label: { Image(systemName: "ellipsis.circle") }
        .accessibilityIdentifier("dictionaryGroupsActions")
        .disabled(busy)
      }
    }
    .task { perform(["operation": "load"]) }
    .alert(renaming == nil ? "新建分组" : "分组名称", isPresented: $naming) {
      TextField("名称", text: $name)
      Button("取消", role: .cancel) {}
      Button("保存") {
        if let renaming { perform(["operation": "rename", "id": renaming, "name": name]) }
        else { perform(["operation": "create", "name": name, "kind": "pinyin"]) }
      }
    }
    .confirmationDialog("删除所选 \(selection.count) 个词库？", isPresented: $confirmsDelete, titleVisibility: .visible) {
      Button("删除", role: .destructive) { perform(["operation": "delete_many", "ids": Array(selection)]) }
    } message: {
      Text("删除分组及其词条；其他已启用分组和原有个人词条会保留。")
    }
    .fileImporter(isPresented: $choosingFile, allowedContentTypes: [.data]) { result in
      do {
        let url = try result.get()
        importFile = url
        importName = url.deletingPathExtension().lastPathComponent
        importFormat = ["yaml", "yml"].contains(url.pathExtension.lowercased()) ? "rime"
          : url.pathExtension.lowercased() == "scel" ? "scel" : "standard"
        importKind = "pinyin"
        importTarget = ""
        importSheet = true
      } catch { message = error.localizedDescription }
    }
    .sheet(isPresented: $importSheet) {
      NavigationStack {
        Form {
          Picker("词库类型", selection: $importKind) {
            ForEach(PersonalWordKind.allCases) { Text($0.title).tag($0.bridgeName) }
          }
          .onChange(of: importKind) { importTarget = "" }
          Picker("文件格式", selection: $importFormat) {
            Text("词在前（TSV）").tag("standard")
            Text("编码在前（TSV）").tag("windows")
            Text("Rime").tag("rime")
            Text("汉字词表").tag("hans")
            Text("搜狗细胞词库").tag("scel")
          }
          Picker("导入到", selection: $importTarget) {
            Text("新建分组").tag("")
            ForEach((document?.collections ?? []).filter { $0.kind == importKind && $0.source.type != "community" }) {
              Text($0.name).tag($0.id)
            }
          }
          if importTarget.isEmpty { TextField("分组名称", text: $importName) }
          if importFormat == "scel" {
            Text("细胞词库只供本机使用，不能发布到社区；只能导入拼音词库及同来源分组。")
          }
        }
        .navigationTitle("导入词库")
        .toolbar {
          ToolbarItem(placement: .cancellationAction) { Button("取消") { importSheet = false } }
          ToolbarItem(placement: .confirmationAction) {
            Button("导入") { startImport() }.disabled(busy)
          }
        }
      }
    }
    .alert("词库分组", isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } })) {
      Button("好", role: .cancel) { message = nil }
    } message: { Text(message ?? "") }
  }

  private func changeSelected(enabled: Bool) {
    perform(["operation": "set_enabled_many", "ids": Array(selection), "enabled": enabled])
  }

  private func startImport() {
    guard let file = importFile, !busy else { return }
    let kind = importKind, format = importFormat, name = importName, target = importTarget
    run {
      let access = file.startAccessingSecurityScopedResource()
      defer { if access { file.stopAccessingSecurityScopedResource() } }
      let bytes = try BoundedFileReader.read(from: file, maximumBytes: format == "scel" ? 8 * 1024 * 1024 : 16 * 1024 * 1024)
      var action: [String: Any] = ["operation": "import", "kind": kind, "format": format, "name": name]
      if !target.isEmpty { action["id"] = target }
      if format == "scel" { action["bytes_base64"] = bytes.base64EncodedString() }
      else {
        guard let text = String(data: bytes, encoding: .utf8) else { throw DictionaryCollectionsBridge.Failure.rejected("invalid") }
        action["text"] = text
      }
      return try DictionaryCollectionsBridge.perform(action)
    }
    importSheet = false
  }

  private func perform(_ action: [String: Any]) {
    run { try DictionaryCollectionsBridge.perform(action) }
  }

  private func run(_ work: @escaping @Sendable () throws -> DictionaryCollectionsBridge.Document) {
    guard !busy else { return }
    busy = true
    Task { @MainActor in
      defer { busy = false }
      do {
        let next = try await Task.detached(priority: .userInitiated, operation: work).value
        document = next
        selection.formIntersection(Set(next.collections.map(\.id)))
        if let report = next.import {
          message = "导入 \(report.imported) 条，跳过重复 \(report.duplicates) 条，失败 \(report.failed) 条。"
            + (report.truncated ? "分组已满。" : "")
        }
        if next.outbox_pending > 0 { try? PersonalDictionaryStore.forSettingsPages().requestRefresh() }
      } catch { message = error.localizedDescription }
    }
  }
}
