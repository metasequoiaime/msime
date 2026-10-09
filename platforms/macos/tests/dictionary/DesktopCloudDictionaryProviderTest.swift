import Foundation
import CryptoKit

@MainActor final class SyntheticSnapshotTarget: DesktopSnapshotTarget {
  let version = "synthetic-local-version"
  func currentVersion() throws -> String { version }
  func stage(_ snapshot: BackendPreparedSnapshot) async throws -> any DesktopSnapshotActivation {
    throw BackendAccountClient.Failure(status: 500)
  }
}

@MainActor final class SyntheticDictionaryAPI: DesktopCloudDictionaryAPI, DesktopSnapshotAPI {
  var calls = 0
  var failure: Int?
  var lastRevision: Int64 = 0
  var lastExport: URL?
  var lastCode = ""
  var lastPosition: Int?
  var lastReplacement: BackendAccountClient.DictionaryValue?
  var lastMode: BackendAccountClient.RankingMode?
  var invalidPage = false
  func dictionaryCatalog(_ kind: BackendAccountClient.DictionaryKind, code: String, offset: Int, scheme: String, profile: String, token: String) async throws -> BackendAccountClient.DictionaryCatalog {
    try tick(); assert((scheme == "shuangpin" && profile == "xiaohe") || (kind == .quick && code.isEmpty && scheme == "pinyin" && profile == "xiaohe"))
    return .init(entries: [.init(kind:kind, code:"he'cheng", word:"合成", weight:100)], offset:invalidPage ? offset + 1 : offset, has_more:offset == 0, revision:0, normalized:"he'cheng")
  }
  func dictionarySnapshot(token: String) async throws -> BackendAccountClient.DownloadedSnapshot {
    assertionFailure("测试不应下载云端快照")
    throw BackendAccountClient.Failure(status: 500)
  }
  func restoreDictionarySnapshot(file: URL, expectedSHA256: String, revision: Int64, token: String) async throws -> BackendAccountClient.SnapshotRestoreResult {
    assertionFailure("已取消的预览不应恢复云端快照")
    throw BackendAccountClient.Failure(status: 500)
  }
  func editCatalog(_ entry: BackendAccountClient.CatalogEntry, revision: Int64, replacement: BackendAccountClient.DictionaryValue?, token: String) async throws -> BackendAccountClient.DictionaryChange {
    try tick(); lastCode = entry.code; lastRevision = revision; lastReplacement = replacement
    return .init(revision:1, previous:nil, replacement:nil)
  }
  func personalCandidates(_ query: BackendAccountClient.CandidateQuery, token: String) async throws -> BackendAccountClient.PersonalCandidates {
    try tick(); assert(query.text == "heig" && query.scheme == "shuangpin")
    return .init(candidates:[.init(code:"heig", word:"合成", weight:100, canonical_pinyin:"he'cheng")], context:"pinyin:he'cheng", revision:0)
  }
  func rankCandidate(_ candidate: BackendAccountClient.PersonalCandidate, query: BackendAccountClient.CandidateQuery, revision: Int64, mode: BackendAccountClient.RankingMode, step: Int, trigger: Int, forceTop: Bool, token: String) async throws -> BackendAccountClient.RankingResult {
    try tick(); lastCode = candidate.mutationCode; lastRevision = revision; lastMode = mode
    assert(step == 3 && trigger == 2 && forceTop)
    return .init(revision:1, changed:true, selection:.init(count:2))
  }
  func removeCandidate(_ candidate: BackendAccountClient.PersonalCandidate, query: BackendAccountClient.CandidateQuery, revision: Int64, token: String) async throws -> BackendAccountClient.DictionaryChange {
    try tick(); lastCode = candidate.mutationCode; lastRevision = revision
    return .init(revision:2, previous:nil, replacement:nil)
  }
  func fixedPositions(context: String, offset: Int, token: String) async throws -> BackendAccountClient.FixedPositions {
    try tick(); return .init(positions:[.init(context:context, code:"he'cheng", word:"合成", position:5)], offset:offset, has_more:false)
  }
  func setFixedPosition(context: String, code: String, word: String, position: Int?, revision: Int64, token: String) async throws -> BackendAccountClient.DictionaryRevision {
    try tick(); lastCode = code; lastRevision = revision; lastPosition = position
    return .init(revision:3)
  }
  func tick() throws { calls += 1; if let failure { throw BackendAccountClient.Failure(status: failure) } }
  func dictionary(_ kind: BackendAccountClient.DictionaryKind, search: String, offset: Int, token: String) async throws -> BackendAccountClient.DictionaryPage {
    try tick()
    return .init(entries: [.init(id: String(repeating: "a", count: 64), kind: kind, code: "synthetic", word: "合成", weight: 100, revision: 17)], has_more: offset == 0, offset: offset)
  }
  func addDictionary(_ kind: BackendAccountClient.DictionaryKind, value: BackendAccountClient.DictionaryValue, token: String) async throws -> BackendAccountClient.DictionaryChange {
    try tick(); return .init(revision: 18, previous: nil, replacement: nil)
  }
  func updateDictionary(_ entry: BackendAccountClient.DictionaryEntry, value: BackendAccountClient.DictionaryValue, token: String) async throws -> BackendAccountClient.DictionaryChange {
    try tick(); lastRevision = entry.revision; return .init(revision: 18, previous: nil, replacement: nil)
  }
  func deleteDictionary(_ entry: BackendAccountClient.DictionaryEntry, token: String) async throws -> BackendAccountClient.DictionaryChange {
    try tick(); lastRevision = entry.revision; return .init(revision: 19, previous: nil, replacement: nil)
  }
  func importDictionary(_ kind: BackendAccountClient.DictionaryKind, text: String, format: BackendAccountClient.DictionaryFileFormat, token: String) async throws -> BackendAccountClient.DictionaryImportResult {
    try tick(); return .init(imported: 1, revision: 20)
  }
  func exportDictionary(_ kind: BackendAccountClient.DictionaryKind, format: BackendAccountClient.DictionaryFileFormat, token: String) async throws -> URL {
    try tick()
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-export-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false, attributes: [.posixPermissions:0o700])
    let file = root.appendingPathComponent("dictionary-" + kind.rawValue + ".tsv")
    let content = String(repeating: "synthetic\t合成\t100\n", count: 150_000).data(using: .utf8)!
    assert(FileManager.default.createFile(atPath: file.path, contents: content, attributes: [.posixPermissions:0o600]))
    lastExport = file
    return file
  }
}

@main enum DesktopCloudDictionaryProviderTest {
  @MainActor static func main() async throws {
    let api = SyntheticDictionaryAPI()
    try await restorationPreviewCancellation(api)
    var provider: BackendCloudDictionaryProvider? = .init(client: api, credentials: { "synthetic-token" })
    try await advanced(provider!, api)
    for kind in ["pinyin", "wubi", "quick", "english"] {
      let page = try await provider!.execute(["operation":"list", "kind":kind, "offset":100, "search":"合成"])
      assert(page["offset"] as? Int == 100 && page["has_more"] as? Bool == false)
    }
    _ = try await provider!.execute(["operation":"add", "kind":"quick", "code":"k2", "word":String(repeating:"界",count:199), "weight":100])
    _ = try await provider!.execute(["operation":"update", "kind":"pinyin", "id":String(repeating:"a",count:64), "revision":17, "code":"he'cheng", "word":"合成", "weight":100])
    assert(api.lastRevision == 17)
    _ = try await provider!.execute(["operation":"delete", "kind":"pinyin", "id":String(repeating:"a",count:64), "revision":18])
    assert(api.lastRevision == 18)
    for format in ["standard", "windows", "hans"] {
      _ = try await provider!.execute(["operation":"import", "kind":"pinyin", "format":format, "text":"he'cheng\t合成\t100\n"])
    }
    let before = api.calls
    for request: NSDictionary in [
      ["operation":"list","kind":"pinyin","offset":true,"search":""],
      ["operation":"list","kind":"pinyin","offset":0.5,"search":""],
      ["operation":"add","kind":"wubi","code":"abcde","word":"合成","weight":1],
      ["operation":"add","kind":"quick","code":"K2","word":"合成","weight":1],
      ["operation":"add","kind":"quick","code":"k2","word":String(repeating:"界",count:200),"weight":1],
      ["operation":"delete","kind":"pinyin","id":"bad/id","revision":17],
      ["operation":"delete","kind":"pinyin","id":String(repeating:"a",count:64),"revision":0],
      ["operation":"export","kind":"pinyin","format":"hans"],
      ["operation":"import","kind":"wubi","format":"hans","text":"合成"],
      ["operation":"import","kind":"pinyin","format":"standard","text":"bad\0"],
      ["operation":"token","kind":"pinyin"]
    ] {
      do { _ = try await provider!.execute(request); assertionFailure("invalid action accepted") } catch { }
    }
    assert(api.calls == before)
    api.failure = 409
    let conflict: NSDictionary = await withCheckedContinuation { continuation in
      _ = provider!.request(["operation":"delete","kind":"pinyin","id":String(repeating:"a",count:64),"revision":17]) { continuation.resume(returning:$0) }
    }
    assert(conflict["ok"] as? Bool == false && conflict["error"] as? String == "conflict")
    api.failure = nil
    let exported = try await provider!.execute(["operation":"export","kind":"pinyin","format":"standard"])
    assert(exported["text"] == nil)
    let descriptor = exported["export_file"] as! [String:Any]
    assert((descriptor["bytes"] as! NSNumber).intValue > 2 * 1024 * 1024)
    let oldFile = api.lastExport!
    _ = try await provider!.execute(["operation":"export","kind":"pinyin","format":"windows"])
    assert(!FileManager.default.fileExists(atPath: oldFile.path))
    let current = api.lastExport!
    provider = nil
    assert(!FileManager.default.fileExists(atPath: current.path))
    var identities = 0
    let changed = BackendCloudDictionaryProvider(client:api, credentials: {
      identities += 1; if identities > 1 { throw CancellationError() }; return "synthetic-token"
    })
    do { _ = try await changed.execute(["operation":"export","kind":"pinyin","format":"standard"]); assertionFailure("old account data exposed") } catch { }
    assert(!FileManager.default.fileExists(atPath: api.lastExport!.path))
    let missing = BackendCloudDictionaryProvider(client:api, credentials: { throw CancellationError() })
    let count = api.calls
    do { _ = try await missing.execute(["operation":"add","kind":"pinyin","code":"he","word":"合","weight":1]); assertionFailure("signed-out write") } catch { }
    assert(api.calls == count)
  }

  @MainActor static func restorationPreviewCancellation(_ api: SyntheticDictionaryAPI) async throws {
    let source = FileManager.default.temporaryDirectory.appendingPathComponent("msime-synthetic-" + UUID().uuidString + ".ndjson")
    defer { try? FileManager.default.removeItem(at: source) }
    let header = #"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":0}"#
    let body = Data((header + "\n").utf8)
    let digest = SHA256.hash(data: body).map { String(format: "%02x", $0) }.joined()
    let footer = #"{"type":"footer","records":1,"sha256":"\#(digest)"}"#
    try (body + Data((footer + "\n").utf8)).write(to: source)
    let snapshots = BackendDesktopSnapshots(client: api, credentials: { "synthetic-token" }, choose: { _ in source })
    let result = try await snapshots.execute(["operation":"snapshot_restore_preview"])
    let token = result["previewToken"] as! String
    let cancelled = try await snapshots.execute(["operation":"snapshot_restore_cancel"])
    assert(cancelled["cancelled"] as? Bool == true)
    do {
      _ = try await snapshots.execute(["operation":"snapshot_restore_native", "token":token])
      assertionFailure("已取消的预览仍可恢复")
    } catch let failure as BackendAccountClient.Failure {
      assert(failure.status == 400)
    }
    let empty = BackendDesktopSnapshots(client: api, credentials: { "synthetic-token" }, choose: { _ in nil })
    let dismissed = try await empty.execute(["operation":"snapshot_restore_preview"])
    assert(dismissed["saved"] as? Bool == false)
  }

  @MainActor static func advanced(_ provider: BackendCloudDictionaryProvider, _ api: SyntheticDictionaryAPI) async throws {
    let catalog: [String:Any] = ["operation":"catalog", "kind":"pinyin", "code":"heig", "offset":100, "scheme":"shuangpin", "profile":"xiaohe"]
    let page = try await provider.execute(catalog as NSDictionary)
    assert(page["revision"] as? Int64 == 0 && page["normalized"] as? String == "he'cheng")
    assert((page["catalog_entries"] as! [[String:Any]])[0]["id"] == nil)
    for kind in ["pinyin", "wubi", "quick", "english"] {
      var request = catalog; request["kind"] = kind
      let response = try await provider.execute(request as NSDictionary)
      assert((response["catalog_entries"] as! [[String:Any]])[0]["kind"] as? String == kind)
    }
    api.invalidPage = true
    do { _ = try await provider.execute(catalog as NSDictionary); assertionFailure("incorrect catalog page accepted") } catch { }
    api.invalidPage = false
    var edit: [String:Any] = ["operation":"edit_catalog", "kind":"pinyin", "code":"he'cheng", "word":"合成", "revision":0, "replacement":NSNull()]
    _ = try await provider.execute(edit as NSDictionary)
    assert(api.lastRevision == 0 && api.lastReplacement == nil && api.lastCode == "he'cheng")
    edit["replacement"] = ["code":"he'cheng", "word":"合成词", "weight":101]
    _ = try await provider.execute(edit as NSDictionary)
    assert(api.lastReplacement?.word == "合成词")
    var query: [String:Any] = ["operation":"candidates", "kind":"jianpin", "text":"heig", "scheme":"shuangpin", "profile":"xiaohe", "limit":100]
    let candidates = try await provider.execute(query as NSDictionary)
    assert((candidates["candidates"] as! [[String:Any]])[0]["canonical_pinyin"] as? String == "he'cheng")
    for kind in ["pinyin", "jianpin", "wubi", "quick", "english"] {
      var request = query; request["kind"] = kind
      _ = try await provider.execute(request as NSDictionary)
    }
    query.merge(["operation":"rank", "code":"he'cheng", "word":"合成", "revision":0, "linear_step":3, "trigger_count":2, "force_top":true]) { _, new in new }
    for mode in BackendAccountClient.RankingMode.allCases {
      query["mode"] = mode.rawValue
      let ranked = try await provider.execute(query as NSDictionary)
      assert(ranked["selection_count"] as? Int64 == 2 && api.lastMode == mode && api.lastCode == "he'cheng" && api.lastRevision == 0)
    }
    let positions = try await provider.execute(["operation":"fixed_positions", "context":"pinyin:he'cheng", "offset":100])
    assert((positions["positions"] as! [[String:Any]])[0]["position"] as? Int64 == 5)
    var fixed: [String:Any] = ["operation":"set_fixed_position", "context":"pinyin:he'cheng", "code":"he'cheng", "word":"合成", "position":5, "revision":1]
    _ = try await provider.execute(fixed as NSDictionary); assert(api.lastPosition == 5)
    fixed["position"] = NSNull()
    _ = try await provider.execute(fixed as NSDictionary); assert(api.lastPosition == nil)
    var removed = query; removed["operation"] = "remove_candidate"
    _ = try await provider.execute(removed as NSDictionary); assert(api.lastCode == "he'cheng")
    let count = api.calls
    for (original, key, value): ([String:Any], String, Any) in [
      (query,"force_top",1), (query,"linear_step",true), (query,"trigger_count",11),
      (query,"revision",-1), (query,"mode","unknown"), (query,"kind","quick"),
      (query,"limit",101), (query,"text","bad\0"), (query,"scheme","unknown"), (query,"profile","unknown"), (fixed,"position",0),
      (fixed,"position",6), (fixed,"position",true), (fixed,"revision",0.5),
      (catalog,"offset",-1), (edit,"replacement","bad")
    ] {
      var invalid = original; invalid[key] = value
      do { _ = try await provider.execute(invalid as NSDictionary); assertionFailure("invalid advanced action accepted") } catch { }
    }
    assert(api.calls == count)
    api.failure = 409
    let conflict: NSDictionary = await withCheckedContinuation { continuation in
      _ = provider.request(query as NSDictionary) { continuation.resume(returning:$0) }
    }
    assert(conflict["error"] as? String == "conflict"); api.failure = nil
    var checks = 0
    let changed = BackendCloudDictionaryProvider(client:api, credentials: {
      checks += 1; if checks > 1 { throw CancellationError() }; return "synthetic-token"
    })
    do { _ = try await changed.execute(catalog as NSDictionary); assertionFailure("stale catalog exposed") } catch { }
    let snapshots = BackendDesktopSnapshots(credentials: { throw CancellationError() }, capture: { SyntheticSnapshotTarget() })
    let status = try await snapshots.execute(["operation":"snapshot_status"])
    assert(status["nativeFiles"] as? Bool == true)
    assert(status["localVersion"] as? String == "synthetic-local-version")
    let cancelled = try await snapshots.execute(["operation":"snapshot_cancel"])
    assert(cancelled["request"] is NSNull)
  }
}
