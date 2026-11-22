import Foundation

/// 只在设置 App 接账号存储；键盘扩展继续使用原有共享词库桥，不加载 Rust 账号适配器。
enum IOSAccountLearningReset {
  @discardableResult
  static func reset(resources: URL? = PersonalDictionaryBridge.packagedResources,
                    stateRoot: URL? = nil, account: BackendRustAccount? = .shared) throws -> Bool {
    guard let account else { throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.failed }
    do {
      let options = try BackendRustAccount.object(
        MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot))
      var localCollectionsRequeued = true
      let result = try account.resetLearnedData(options: options) {
        localCollectionsRequeued = try MetasequoiaInputSessionBridge.resetLearnedData(resources: resources, stateRoot: stateRoot)
        return ["reset": true]
      }
      return localCollectionsRequeued && result["collections_error"] == nil
    } catch let failure as MetasequoiaInputSessionBridge.LearnedDataResetFailure {
      throw failure
    } catch let failure as BackendRustClient.Failure where failure.code == "learning_busy" {
      throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.busy
    } catch {
      throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.failed
    }
  }
}
