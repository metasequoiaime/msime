# Agent Note: 设备测试包按引用自动收集应用源文件

Status: implemented

## Problem

设备测试包（`tests/device/build-editor.sh` 打包，`check-host.sh` 只做编译检查）原来编译一份手写清单 `tests/device/editor-sources.txt`：测试类，加上它们直接和间接引用到的应用类。手写清单是 [2026-10-08 的决定](2026-10-08-compile-gates-for-device-suite-and-arkts.md)，理由是应用源文件按功能分在子目录里，`package` 却大多是 `app.msime.android`，`javac -sourcepath` 按包路径找不到它们。

代价比当时估的大。改设备测试的 PR 几乎都要改这份清单，它成了 develop 上改动最频繁的文件之一，并行的 Android PR 常在同一处冲突。应用代码挪走后，没人删的条目也一直留着：清单里攒下了十几个已经没有任何测试类引用的应用类，还有重复条目。

## Decision

- `tests/device/compile-sources.sh` 编译设备测试包的 Java，`build-editor.sh` 和 `check-host.sh` 都调用它，清单删除。
- `tests/device/` 下的每个 `.java` 都是入口。脚本在 `target/android/` 下建一个临时目录，把 `platforms/android/java`、`platforms/android/tests`（不含 `device/`）和 OCR 合成夹具里的每个 `.java` 按自己 `package` 行的包路径放成符号链接，交给 `javac -sourcepath`，并用 `-implicit:class`：只有入口直接或间接引用到的类会被编译，测试包不会带上整个应用。没有 `package` 行的 JVM 冒烟在默认包里，引用不到，不进视图。
- 视图只收 `.java`。设备测试引用到 Kotlin 类、或只有 Gradle 才能编译的类（引用 AndroidX 等），照样在编译时失败。同一个包里出现两个同名源文件时脚本直接失败；编译报错里的视图路径换回仓库路径。
- `scripts/test-android-app-version-policy.py` 里「清单必须包含 `AppVersionPolicy`」一条随清单删除：设备测试用到它时，编译自然会带上。

## 与改动前的差异

在 develop `db907e941f` 上用旧清单和新脚本各编译一次，比较产出的类文件：

- 两边都有的类文件逐字节相同。在这之前合入的 #11527 往清单里补了 `AccountLearningSessionStore` 和 `LearningKeystoreSupport`，新脚本不用改任何文件就收进了它们。
- 新脚本不再编译以下没有被引用的应用类（连同内部类）：`AccountNamePolicy`、`ClipboardEditKey`、`DeviceDataApi`、`DigestPolicy`、`LayoutPolicy`、`LetterHintTable`、`NineKeySwipePolicy`、`RelativeTimePolicy`、`SetPolicy`、`SuperscriptShortcutPolicy`、`SwipeHintPolicy`、`SyncSwitch`、`ThreadPolicy`。
- 新脚本多编译了 `tests/device/fixtures/OcrCameraImage.java`：清单原来没收它，现在设备目录下的文件都是入口。它没有被任何测试引用。

这些类里，`AccountNamePolicy`、`DeviceDataApi`、`LetterHintTable` 由目标上下文的类加载器按名字加载，`ClipboardEditKey` 由 `Class.forName` 加载，四处所在的 instrumentation 都以产品包为目标。改动后它们取到产品 APK 里的那一份，和同一个测试里早就这样加载的 `app.msime.android.home.HostTask` 一样。设备套件里按名字加载产品类的地方都依赖产品 APK 的 R8 规则保留 `app.msime.android.**` 的全部类（#11404）。

## Alternatives considered

- **继续手写清单**（2026-10-08 的选择）：最强的理由是清单显式，编进测试包的每个应用类都看得见，漏类由 CI 编译兜住。没有继续：每加一个设备测试、或测试用到的应用类挪进新的辅助类，都要改同一个文件，并行 PR 互相冲突；多余的条目没有任何检查，已经攒下十几个。
- **编译全部可编译的应用源文件**（JVM 冒烟的选法：`home/` 以外、不引用 AndroidX 的文件）：不需要视图，最简单。没有采用：以产品包为目标的 instrumentation 里，测试包的类排在产品的类前面，测试包带上的每个应用类都会顶替产品自己的那一份。现在只顶替测试真正用到的几十个类，整树编进去会扩大到几百个。
- **让目录与包一致**（每个子目录用自己的包名）：之后 `-sourcepath` 直接可用，目录也成了真正的边界。没有采用：要改几乎所有应用文件的 `package`、`import` 和可见性，而 `MSIMEInputService`、启动器别名、`NativeClient`（JNI 符号名）这些类的全限定名不能动；为一个测试构建问题动整个源码树不划算。

## Consequences

- **收益**：新增设备测试，或者把设备测试用到的代码挪进新类，都不用再改任何清单。
- **代价**：每次编译多建一份符号链接视图（每个应用源文件一个链接），整个脚本在开发机上只要几秒。测试包里有哪些应用类不再写在一个文件里，要看就看编译出的类。
- **仍然没有覆盖**：按名字反射查找的产品类和成员仍不经编译检查，只能靠设备运行报出（见 [语音自动插入设备验收的反射签名与失败输出](2026-10-29-voice-auto-insert-device-reflection.md)）。

## Verification

基于 develop `db907e941f`：

- 上面「与改动前的差异」一节的类文件比较。
- `bash platforms/android/check-host.sh`：JVM 冒烟通过，设备套件编译和 `NativeClient` 可达性检查通过；`bash platforms/android/tests/device/build-editor.sh` 打出并签名测试包。

同一改动在变基前的 develop `feb94ae837` 上：

- 失败路径：临时加一个引用 `home/HomeActivity` 的设备测试，编译失败，报错指向 `platforms/android/java/app/msime/android/home/HomeActivity.java`；临时加一个同包同名的源文件，脚本报出两个文件的路径。
- 模拟器 API 35（x86_64，开发密钥的 full 包）上 `smoke.sh --pr`、`--risk`、`--key-faces` 通过，其中包括按名字反射加载上面那几个类的设备测试；API 28 上 `smoke.sh --core` 通过。
- `MSIME_VERIFY_ALL=1 bash scripts/verify-local.sh --quick` 通过（与 CI 相同的字体环境；没有 macOS、Windows 交叉编译和 HarmonyOS 工具链的阶段跳过）。
