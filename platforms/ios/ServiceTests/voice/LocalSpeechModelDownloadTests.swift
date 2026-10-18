import XCTest

final class LocalSpeechModelDownloadTests: XCTestCase {
  func testCancellationBeforeStartingSkipsTheInstaller() {
    let download = LocalSpeechModelDownload(id: "x-asr-zh-en-offline")
    download.cancel()
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    XCTAssertThrowsError(try download.install(root: root, mirror: "") { _ in
      XCTFail("启动前取消不应调用共享安装器或报告进度")
    }) { error in
      XCTAssertEqual((error as? LocalSpeechModelStore.HostFailure)?.code, "local_model_cancelled")
    }
    XCTAssertFalse(FileManager.default.fileExists(atPath: root.path))
  }

  func testFirstRegisteredProgressCanCancelDownloadAndImport() {
    let sources: [[URL]?] = [nil, []]
    for files in sources {
      let download = LocalSpeechModelDownload(id: "x-asr-zh-en-offline")
      let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
      var observed: [LocalSpeechInstallProgress] = []
      // 即使首次回调回归，明文合成镜像也会在联网前被拒绝。
      XCTAssertThrowsError(try download.install(root: root, mirror: "http://synthetic.invalid/", files: files) { update in
        observed.append(update)
        download.cancel()
      }) { error in
        XCTAssertEqual((error as? LocalSpeechModelStore.HostFailure)?.code, "local_model_cancelled")
      }
      XCTAssertEqual(observed.count, 1)
      XCTAssertEqual(observed.first?.stage, files == nil ? "download" : "import")
      XCTAssertEqual(observed.first?.downloaded, 0)
      XCTAssertFalse(FileManager.default.fileExists(atPath: root.path))
    }
  }
}
