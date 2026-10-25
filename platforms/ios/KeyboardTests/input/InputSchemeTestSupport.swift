import XCTest

// InputSchemePreference.scheme silently substitutes enabledSchemes[0] for a scheme the user has
// hidden, so a test that assigns a scheme without first enabling it gets whatever the simulator
// happened to have left in the app group. That state outlives a test bundle: a UI test that hides
// a scheme and fails before restoring it turned every nine-key assertion in this target red on CI
// while the same commit stayed green locally. Tests that depend on a scheme claim the whole set.
extension XCTestCase {
  func enableAllInputSchemes() {
    let previous = InputSchemePreference.enabledSchemes
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    // 新建的键盘先把共享文档里记的方案抄进 App Group 镜像再按它行事，而前面的用例经键盘方案卡片选过的方案会留在模拟器共用的那份文档里，盖过用例给镜像赋的方案。用例期间文档里只记全部方案都启用、不记选中的入口，让镜像决定选中哪个，结束后放回原样。不能直接删掉这一项：client-core 读到没有启用列表的文档时按改成只有中文之前的默认列表补上，粤拼、注音、越南语、藏文和笔画就不在里面。
    let selection = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_keyboard_schemes"]
    _ = MetasequoiaInputSessionBridge.updateSharedPreferences {
      $0["touch_keyboard_schemes"] = ["enabled": ChineseInputScheme.allCases.map(\.sharedIdentifier)]
    }
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_keyboard_schemes"] = selection }
      InputSchemePreference.enabledSchemes = previous
    }
  }

  /// 离线释义开关同理以共享文档为准：新建的键盘先把文档里的 `candidate_english_gloss` 抄进 App Group 镜像，只给 `CandidateGlossPreference.enabled` 赋值的用例会被文档盖掉（文档默认关）。这里文档和镜像一起写，用例结束后两边都放回原样。已经建好的键盘在下一次出现之前仍按镜像行事，所以用例中途只改镜像来模拟「刚关掉」仍然可以。
  func setCandidateGloss(_ enabled: Bool) {
    let previousDocument = MetasequoiaInputSessionBridge.loadSharedPreferences()?[InputHabitPreference.glossKey]
    let previousMirror = CandidateGlossPreference.enabled
    _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[InputHabitPreference.glossKey] = enabled }
    CandidateGlossPreference.enabled = enabled
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[InputHabitPreference.glossKey] = previousDocument }
      CandidateGlossPreference.enabled = previousMirror
    }
  }
}
