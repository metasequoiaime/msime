import Foundation

/// 只在设置 App 接账号存储；键盘扩展继续使用原有共享词库桥，不加载 Rust 账号适配器。
enum IOSAccountLearningReset {
  static func reset(resources: URL? = PersonalDictionaryBridge.packagedResources,
                    stateRoot: URL? = nil, account: BackendRustAccount? = .shared) throws {
    guard let account else { throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.failed }
    do {
      let options = try BackendRustAccount.object(
        MetasequoiaInputSessionBridge.preparedHostOptions(resources: resources, stateRoot: stateRoot))
      _ = try account.resetLearnedData(options: options) {
        try MetasequoiaInputSessionBridge.resetLearnedData(resources: resources, stateRoot: stateRoot)
        return ["reset": true]
      }
    } catch let failure as MetasequoiaInputSessionBridge.LearnedDataResetFailure {
      throw failure
    } catch let failure as BackendRustClient.Failure where failure.code == "learning_busy" {
      throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.busy
    } catch {
      throw MetasequoiaInputSessionBridge.LearnedDataResetFailure.failed
    }
  }
}
