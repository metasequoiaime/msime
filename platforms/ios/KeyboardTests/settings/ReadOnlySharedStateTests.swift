import XCTest
import Darwin

/// 键盘扩展没有「允许完全访问」时 App Group 只读（#10945）。模拟器上的扩展照样写得进 App Group，所以这里把共享状态目录的权限位改成只读来模拟真机，再走桥的只读准备。
@MainActor
final class ReadOnlySharedStateTests: XCTestCase {
  private var root: URL!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-read-only-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    setTree(root, writable: true)
    try? FileManager.default.removeItem(at: root)
  }

  /// App 准备过共享目录之后，只读的桥用它的代次：打 nihao 出「你好」并上屏，会话持着只读租约挡住 App 的维护，收起后放掉；共享目录一个字节也没变。改前同样的桥在准备时就失败（偏好锁和日志回放都要写）。
  func testAReadOnlyBridgeTypesOnTheGenerationTheAppPrepared() throws {
    let state = root.appendingPathComponent("group/MSIME", isDirectory: true)
    let app = MetasequoiaInputSessionBridge(stateRoot: state, startsSuspended: true)
    XCTAssertFalse(app.initializationFailed, "precondition: the writable bridge prepares the shared state")
    XCTAssertFalse(app.sharedStateIsReadOnly)
    let locks = try lockFiles(stateRoot: state)
    XCTAssertTrue(locks.allSatisfy { FileManager.default.fileExists(atPath: $0) },
                  "the writable preparation leaves both lease lock files for a read-only host")
    let before = try listing(state)
    setTree(state, writable: false)
    try XCTSkipIf(FileManager.default.createFile(atPath: state.appendingPathComponent("probe").path, contents: nil),
                  "file modes do not stop writes here")
    XCTAssertTrue(MetasequoiaInputSessionBridge(stateRoot: state, startsSuspended: true).initializationFailed,
                  "precondition: the writable preparation fails on a read-only state root")

    let bridge = MetasequoiaInputSessionBridge(
      stateRoot: state, startsSuspended: true, sharedStateWritable: false,
      privateRoot: root.appendingPathComponent("extension/MSIME-read-only", isDirectory: true))
    XCTAssertFalse(bridge.initializationFailed)
    XCTAssertTrue(bridge.sharedStateIsReadOnly)
    try bridge.resumeDictionarySession()
    XCTAssertFalse(locks.allSatisfy(exclusiveLockAvailable), "the read-only session holds the shared lease")
    var snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertNil(snapshot.diagnosticText)
    let index = try XCTUnwrap(snapshot.candidates.firstIndex(of: "你好"), "\(snapshot.candidates)")
    XCTAssertEqual(bridge.selectCandidate(at: UInt(index)).commitText, "你好")
    XCTAssertTrue(bridge.suspendDictionarySession())
    XCTAssertTrue(locks.allSatisfy(exclusiveLockAvailable), "suspending hands the read-only lease back")
    setTree(state, writable: true)
    XCTAssertEqual(try listing(state), before, "the read-only bridge wrote into the shared state")
  }

  /// App 从没运行过、共享目录里什么也没有时，只读的桥用扩展随包的词库照样打得出字，也不在共享目录里建任何东西。
  func testAReadOnlyBridgeWithoutSharedStateTypesOnTheBundledDictionaries() throws {
    let group = root.appendingPathComponent("group", isDirectory: true)
    try FileManager.default.createDirectory(at: group, withIntermediateDirectories: true)
    setTree(group, writable: false)
    let state = group.appendingPathComponent("MSIME", isDirectory: true)
    let bridge = MetasequoiaInputSessionBridge(
      stateRoot: state, startsSuspended: true, sharedStateWritable: false,
      privateRoot: root.appendingPathComponent("extension/MSIME-read-only", isDirectory: true))
    XCTAssertFalse(bridge.initializationFailed)
    try bridge.resumeDictionarySession()
    var snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertNil(snapshot.diagnosticText)
    XCTAssertTrue(snapshot.candidates.contains("你好"), "\(snapshot.candidates)")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    XCTAssertFalse(FileManager.default.fileExists(atPath: state.path))
  }

  /// 只读的桥不改词库：置顶、删除、固定排位和取消固定都被运行时拒绝，组字和候选原样留着，随包的 EngineResources 一个文件也没变。只读布局下词库就是它，改前置顶和删除直接改写它，模拟器上之后的资源校验全部失败。
  func testAReadOnlyBridgeRefusesCandidateEditsAndKeepsTheComposition() throws {
    let group = root.appendingPathComponent("group", isDirectory: true)
    try FileManager.default.createDirectory(at: group, withIntermediateDirectories: true)
    setTree(group, writable: false)
    let bridge = MetasequoiaInputSessionBridge(
      stateRoot: group.appendingPathComponent("MSIME", isDirectory: true), startsSuspended: true,
      sharedStateWritable: false,
      privateRoot: root.appendingPathComponent("extension/MSIME-read-only", isDirectory: true))
    XCTAssertFalse(bridge.initializationFailed)
    XCTAssertTrue(bridge.sharedStateIsReadOnly)
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let before = try listing(resources)
    try bridge.resumeDictionarySession()
    var snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    let candidates = snapshot.candidates
    let index = try XCTUnwrap(candidates.firstIndex { $0.count > 1 }, "\(candidates)")
    for action in [MetasequoiaCandidateAction.promote, .remove, .fix(position: 1), .clearPosition] {
      let edited = bridge.editCandidate(at: UInt(index), expectedWord: candidates[index], action: action)
      XCTAssertEqual(edited.diagnosticText, "candidate editing needs write access to the shared state", "\(action)")
      XCTAssertEqual(edited.candidates, candidates, "\(action)")
    }
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    XCTAssertEqual(try listing(resources), before, "the read-only bridge rewrote the bundled dictionaries")
  }

  /// 可写的目录：试写成功，探测文件随即删掉，目录里什么也不留。
  func testTheWriteProbeSucceedsInAWritableDirectoryAndLeavesNothing() throws {
    let container = root.appendingPathComponent("container", isDirectory: true)
    try FileManager.default.createDirectory(at: container, withIntermediateDirectories: true)
    XCTAssertTrue(MetasequoiaInputSessionBridge.sharedStateIsWritable(in: container))
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: container.path), [])
  }

  /// 容器还不存在时先建出来再试（取不到 App Group 时容器是应用自己的 Application Support，第一次可能还没有）。
  func testTheWriteProbeCreatesAMissingContainer() throws {
    let container = root.appendingPathComponent("absent/container", isDirectory: true)
    XCTAssertTrue(MetasequoiaInputSessionBridge.sharedStateIsWritable(in: container))
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: container.path), [])
  }

  /// 只读的目录（真机上没有完全访问时的 App Group）：试写失败，报不可写，目录不变。
  func testTheWriteProbeFailsInAReadOnlyDirectory() throws {
    let container = root.appendingPathComponent("container", isDirectory: true)
    try FileManager.default.createDirectory(at: container, withIntermediateDirectories: true)
    XCTAssertEqual(chmod(container.path, 0o555), 0)
    defer { chmod(container.path, 0o755) }
    try XCTSkipIf(FileManager.default.createFile(atPath: container.appendingPathComponent("probe").path, contents: nil),
                  "file modes do not stop writes here")
    XCTAssertFalse(MetasequoiaInputSessionBridge.sharedStateIsWritable(in: container))
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: container.path), [])
  }

  // MARK: - helpers

  private func lockFiles(stateRoot: URL) throws -> [String] {
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let options = try XCTUnwrap(JSONSerialization.jsonObject(
      with: MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot)) as? [String: Any])
    return try ["user_data", "dictionaries"].map {
      (try XCTUnwrap(options[$0] as? String, $0) as NSString).appendingPathComponent(".msime-dictionary-access.lock")
    }
  }

  /// 能否立刻取得这个锁文件的独占 flock。
  private func exclusiveLockAvailable(_ path: String) -> Bool {
    let descriptor = open(path, O_RDONLY | O_CLOEXEC)
    guard descriptor >= 0 else { return errno == ENOENT }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { return false }
    flock(descriptor, LOCK_UN)
    return true
  }

  /// 整棵目录树的路径、大小和修改时间。
  private func listing(_ directory: URL) throws -> [String] {
    let keys: [URLResourceKey] = [.fileSizeKey, .contentModificationDateKey]
    let enumerator = try XCTUnwrap(FileManager.default.enumerator(at: directory, includingPropertiesForKeys: keys))
    var entries: [String] = []
    for case let url as URL in enumerator {
      let values = try url.resourceValues(forKeys: Set(keys))
      entries.append("\(url.path) \(values.fileSize ?? -1) \(values.contentModificationDate?.timeIntervalSince1970 ?? 0)")
    }
    return entries.sorted()
  }

  /// 把整棵目录树改成只读（目录 0555、文件 0444）或改回可写。
  private func setTree(_ directory: URL, writable: Bool) {
    let manager = FileManager.default
    var paths = [directory.path]
    if writable { chmod(directory.path, 0o755) }
    if let enumerator = manager.enumerator(atPath: directory.path) {
      for case let relative as String in enumerator {
        let path = (directory.path as NSString).appendingPathComponent(relative)
        var isDirectory: ObjCBool = false
        manager.fileExists(atPath: path, isDirectory: &isDirectory)
        if isDirectory.boolValue && writable { chmod(path, 0o755) }
        paths.append(path)
      }
    }
    for path in paths.reversed() {
      var isDirectory: ObjCBool = false
      manager.fileExists(atPath: path, isDirectory: &isDirectory)
      chmod(path, isDirectory.boolValue ? (writable ? 0o755 : 0o555) : (writable ? 0o644 : 0o444))
    }
  }
}
