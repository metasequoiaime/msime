# 新版整句候选与模型

Rust 新版仍支持本地整句候选、神经模型重排和 octagram `.gram` 语法模型，但各平台的设置入口尚未完全一致。旧版 Windows 截图中的开关不能逐项照搬到新版。

本文对应 [#9457](https://github.com/metasequoiaime/msime/issues/9457)，说明仓库当前实现；已经安装的发行版可能早于这些改动。

## 旧版功能在新版中的位置

| 旧版截图中的项目 | 新版的行为与配置 |
| --- | --- |
| Trigram / Unigram 模型整句 | 本地词格组合词库生成整句，结合随词库提供的 n-gram 数据打分；设置为「本地整句联想」，没有保留这两个独立开关 |
| 神经网络模型整句（效果优先） | 「桌面神经联想」，偏好 `sentence_association.neural_desktop`；输入停顿后由桌面运行时重排，模型资源包为 `settled-model` |
| 神经网络模型整句（速度优先） | 偏好 `sentence_association.neural_keyboard`，使用键盘神经模型；手机提供该能力的版本通过「整句联想 → 增强」启用 |
| 去重后补充次选 | 偏好 `sentence_association.show_next_on_duplicate`，共享桌面设置页与 Windows 原生设置页提供同名开关 |
| octagram 语法模型 | 偏好 `sentence_association.grammar_model`，共享桌面设置页与 Windows 原生设置页提供「语法模型」选择及资源包下载、删除 |

本地词格默认开启，神经重排与语法模型默认关闭。没有安装可选模型时，本地整句仍可生成。语法模型参与本地解码打分，神经模型负责重排；它们不要求登录或调用云端 AI 服务。

## 各平台如何设置

### macOS、Linux

打开产品的设置应用，进入「输入 → 候选与联想」：

1. 「更多选项」中的「本地整句联想」控制本地整句候选。
2. 打开「桌面神经联想」会按需下载桌面模型，下面显示资源包状态。
3. 在「语法模型」选择一种模型，然后在下面的资源包行完成下载。选择模型本身不等于下载完成。
4. 模型安装完成后切换一次输入框，使输入会话读取新的模型路径；仍未生效时重新启动输入法。

语法模型一次选择一种，也可选「不使用」。已安装模型可以删除；下载失败时查看资源包行的错误和下载镜像入口。

打开「桌面神经联想」后仍看不出效果时，先确认输入会话有没有接上模型：在设置应用「维护与诊断」页的「诊断日志」打开输入法日志后，重新启动输入法（同一页的「立即重启输入法服务」，或注销后重新登录），然后在任意输入框里打几个字，再看 `diagnostic.log`（macOS）或 stderr（Linux）里的 `settled model ...` 行（macOS 记为 `host_api: settled model ...`）。只切换输入框不一定会写这一行：已经打开的输入会话不会重新报告。开关开着时，会话建立、开关打开和模型装好或移除后换上时各写一行，开关关着时不写：

| 日志行 | 含义 |
| --- | --- |
| `settled model attached` | 模型已加载并挂在会话上，停顿后的整句重排会用它 |
| `settled model not loaded` | 找到了模型文件但没能用上；同一时间附近的 `sentence model unreadable`（文件读不了，例如超过 64 MiB、是符号链接或没有权限）或 `sentence model ignored`（文件损坏或版本不符）说明原因，重新下载资源包 |
| `settled model missing` | 没有随包的模型，状态目录下也没有已安装的 `settled-model` 资源包，在设置页下载 |
| `settled model missing without state directory` | 没有随包的模型，而输入会话没拿到状态目录，下载的资源包不会被找到；请附上日志报告问题。只会出现在 Linux 的 stderr 和 Windows 的 `logs\server.log` 里，见表下说明 |

macOS 上看不到 `settled model missing without state directory` 这一行：`diagnostic.log` 就写在这个状态目录里，输入会话没拿到状态目录时诊断日志整个不会写，表现与日志没打开一样。反过来，只要 `diagnostic.log` 里有 `focus_in`、`preferences_applied` 或 `host_api:` 开头的行，输入会话就拿到了状态目录，可以排除「没拿到状态目录」这种情况。

模型已接上不表示每次停顿都会换首选：可信的词典首选等情况会跳过重排，首选没变本身不能说明模型没有加载。模型文件的访问时间（atime）也不能作为依据，APFS 上普通读取不一定更新它。

### Windows

新版使用原生设置窗口，入口与 macOS、Linux 相同，在「输入 → 候选与联想」：

1. 「更多选项」中的「本地整句联想」控制本地整句候选。
2. 打开「桌面神经联想」后，下面出现「神经联想模型」行，点「下载」安装 `settled-model`。安装包不带这个模型，不下载时开关不起作用。
3. 在「语法模型」选择一种模型，再在下面的「模型文件」行点「下载」。下载中可以取消，已安装的可以删除。
4. 模型安装完成后切换一次输入框；仍未生效时重新启动输入法。

打开「桌面神经联想」后仍看不出效果时，在「维护与诊断」打开「Server 端日志」后点「立即重启输入法服务」，然后在任意输入框里打几个字，`logs\server.log` 里会出现 `host_api: settled model ...` 行（只有类别，不带路径），含义见上文 macOS、Linux 一节的表格。只切换输入框不一定会写这一行：同一个应用里已经打开的输入会话不会重新报告。

Windows 0.3.0 及更早版本的原生设置页只有「本地整句联想」，没有上面第 2、3 步的行；这两项入口从 0.3.0 之后的版本开始提供。在这些版本上需要使用模型时，可以按下文的开发工具流程操作。

旧版配置文件及其模型目录不会仅因安装新版而自动成为新版已安装的资源包，需要在设置页重新下载。

### Android、HarmonyOS、iOS

Android、HarmonyOS 手机和 iOS 在原生宿主的「表达 → 智能 → 整句联想」选择：

- 「关闭」：不生成本地整句候选。
- 「标准」：使用本地词格。
- 「增强」：本地词格加键盘神经模型；版本未附带键盘模型时不提供这一档。

上述移动端目前都没有共享桌面页的语法模型下载管理入口。「增强」也不表示下载了万象或八股文模型。

## 模型如何更新

| 数据或模型 | 更新方式 |
| --- | --- |
| 中文词库、bigram/trigram 数据与随词库提供的键盘神经模型 | 使用设置中的「词库更新」获取当前引擎兼容的官方资源；版本是否包含键盘模型取决于产品版本能力 |
| 桌面神经模型 | 通过 `settled-model` 资源包安装；安装包默认不内置，开发构建可内置 |
| 万象、八股文语法模型 | 通过所选语法模型的资源包安装；下载版本由本应用内的锁文件固定 |

「词库更新」不会顺便更新所有可选 `.gram` 模型。应用也不会直接跟随上游仓库的最新模型文件。升级应用后，如果新版本锁定的模型字节发生变化，资源包会显示需要更新，重新下载即可；字节未变时不因下载地址变化而重下。

当前桌面界面提供以下语法模型，大小按 MiB 近似：

| 界面名称 | 偏好值 | 资源包 ID | 大小 |
| --- | --- | --- | --- |
| 万象 LTS | `wanxiang` | `grammar-model-wanxiang` | 380 MiB |
| 八股文·词级 | `baguwen_word` | `grammar-model-baguwen` | 197 MiB |
| 八股文·词级紧凑 | `baguwen_compact` | `grammar-model-baguwen-compact` | 39 MiB |

白霜/墨奇虽有实验性数据及读取支持，目前不在共享桌面选择器中提供。模型的来源、长度和 SHA-256 见 [语法模型锁](../resources/grammar-models.lock.json)，许可见[第三方组件清单](third-party.md#octagram-语法模型查询)。

## 使用开发工具安装和配置

此流程从本仓库源码运行现有资源包安装器，需 Rust 工具链和网络。先确定**当前使用的产品版本**在 `runtime-options.json` 中记录的绝对 `preferences_directory`。它对应宿主读取资源包的状态根；不要根据旧版目录或别的版本猜路径。

例如在 PowerShell 中，为该目录安装八股文紧凑模型：

```powershell
cargo run --locked -p msime-client-providers --example install_resource_pack -- 'C:\Example\MSIME-State' grammar-model-baguwen-compact
```

将示例路径替换为该产品实际使用的状态根。安装器按编译时锁定的长度和 SHA-256 校验文件，完成后发布到 `resource-packs/grammar-model-baguwen-compact/`，并写入完整安装标记；不要只把一个 `.gram` 文件复制进去。

退出设置应用和输入法后，备份该状态根的 `preferences.json`，在其已有 `preferences.sentence_association` 对象中设置以下字段，保留其余字段及文档外层结构：

```json
{
  "word_lattice": true,
  "grammar_model": "baguwen_compact"
}
```

重新启动输入法，输入多个拼音音节使用本地整句候选。选中模型但资源包没有完整装在同一状态根时，会退回无语法模型的行为。停止使用时，将 `grammar_model` 改成 `none`；本地整句开关可以继续保留为 `true`。

桌面神经模型使用同一个安装器，资源包 ID 换成 `settled-model`，相应开关为 `neural_desktop: true`。源码工具与已安装产品必须使用兼容的资源锁；仓库安装器准备的最新资源不保证能被较旧的发行版识别。此流程不提供自选任意 `.gram` 或 `.safetensors` 的导入功能，也不会自动训练个人模型。

## 实现与验收边界

- 共享桌面控件：[sentence-association-section.tsx](../packages/ui/src/settings/sentence-association-section.tsx)。
- Windows 原生输入设置：[main.cpp](../platforms/windows/settings/main.cpp) 的 `build_input_habits`、`build_neural_desktop_settings` 与 `build_sentence_model_settings`。
- 宿主模型装配：[host-api](../crates/host-api/src/lib.rs) 的 `engine_sentence_association`。
- 下载、校验与安装状态：[resource_packs.rs](../crates/client-providers/src/resource_packs.rs)。

这些入口用于核对当前能力。源码支持、资源包安装成功和 Windows 真实编辑器中的整句效果属于不同的验证范围。
