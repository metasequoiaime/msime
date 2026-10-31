# 水杉輸入法

[简体中文](../../README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · **繁體中文**

> 本文是[簡體中文 README](../../README.md) 的翻譯，內容以簡體中文版為準。連結的文件多為簡體中文。

### 打中文，候選旁就是英文。

開啟「顯示英文釋義」後，查得到釋義的候選旁會標出英文。在電腦上再開啟「候選詞翻譯」，按 Ctrl+Enter 上屏的就是高亮候選的英文：只有一條釋義時直接上屏，有幾條時先列出來讓你選。

```text
你打的       候選窗裡            按 Ctrl+Enter 上屏
yusuan   →  预算   budget    →  budget
zhoumo   →  周末   weekend   →  weekend
```

**[下載水杉輸入法](https://msime.app/download/)** · Windows · macOS · Linux · Android · iOS · HarmonyOS · 免費開源（GPL-3.0-only）

- **怎麼開啟**：預設都是關的。在設定裡開啟「顯示英文釋義」（Android 和 iOS 上叫「離線英文釋義」），想看音標再開「顯示讀音」（Android 上叫「顯示英文音標」）。釋義和讀音查本機詞庫，顯示時不連網（Android 的英文音標詞典要在第一次開啟時下載一次）；「候選詞翻譯」不選線上翻譯服務時也只查本機詞庫。
- **還有什麼**：「候選詞翻譯」的目標語言還可以換成法、日、西、俄、德、韓（鴻蒙手機上沒有俄語）；「背單詞」內建中考到 GRE 八本詞書，按間隔重複排複習。
- **平台差異**：音標目前在 macOS、Android 和 Windows 新版預覽上顯示；Windows 正式版由 [msime-windows](https://github.com/metasequoiaime/msime-windows) 發布，沒有單獨的「顯示英文釋義」開關，開啟「候選詞翻譯」就會顯示離線英文釋義，也能用 Ctrl+Enter，暫時沒有音標，也沒有按間隔重複排複習的「背單詞」詞書；手機版仍是測試版。

**輸入法能看到你輸入的一切，所以這個問題應該有一個能逐條核對的答案：[哪些資料會離開裝置](../../PRIVACY.md)。**簡短版本：新安裝的預設設定下沒有功能會把你輸入的內容傳出裝置；雲端聯想（把正在組的拼音傳給 Google 輸入工具）要你在首次詢問或設定裡開啟，其餘連網功能都要你自己填寫憑證或選擇服務才會運作。另有一條不攜帶輸入內容的自有匿名使用統計：六個平台都會向 `api.msime.app` 傳送每日活躍、工作階段和當機（錯誤摘要與去掉目錄的呼叫堆疊）紀錄，預設開啟，可以在設定的「匿名使用統計」裡關閉——欄位、逐平台差異和寫入磁碟的位置逐條列在 [PRIVACY.md](../../PRIVACY.md#使用统计与崩溃上报默认开启可关闭)。儲存庫不接入任何第三方統計或當機回報 SDK。

水杉輸入法（MSIME）是面向 Android、iOS、macOS、Linux、Windows 與 HarmonyOS 的多平台中文輸入法。六個平台各有自己的原生輸入法宿主，都接入同一套共用輸入執行環境；設定介面按平台歸屬：macOS、Linux 與 HarmonyOS 用同一份共用 React 設定頁，Windows、iOS 與 Android 用各自的原生設定介面，設定的業務規則應當放在共用的 Rust 層、各平台介面只做介面（目前還有部分規則寫在原生介面裡）；React 設定頁在 macOS 與 Linux 上透過 Tauri 呼叫一般的 Rust 業務函式庫，在 HarmonyOS 上不經過 Tauri，由 ArkWeb 載入、經 ArkTS 原生橋與 NAPI 呼叫 Rust；輸入演算法由 msime-engine 提供。

專案首頁：[msime.app](https://msime.app/) · [GitHub 儲存庫](https://github.com/metasequoiaime/msime) · [貢獻者](https://github.com/metasequoiaime/msime/graphs/contributors) · [問題回報](https://github.com/metasequoiaime/msime/issues) · [程式碼簽章政策 / Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)

> **關於名稱**：MSIME 是 Metasequoia IME（水杉輸入法）的縮寫，與 Microsoft IME 無關，也與微軟沒有任何關聯。程式碼、套件名稱和儲存庫名稱中的 `msime` 一律是這個含義。設定中的 `shuangpin_profile: microsoft` 是「微軟雙拼」方案，與小鶴、自然碼、首道並列的四個鍵位方案之一，供習慣該鍵位的使用者選擇，同樣不代表任何關聯。

[![Core CI](https://img.shields.io/github/actions/workflow/status/metasequoiaime/msime/ci.yml?branch=develop&label=Core%20CI&logo=githubactions&logoColor=white)](https://github.com/metasequoiaime/msime/actions/workflows/ci.yml?query=branch%3Adevelop)
[![Native Platform CI](https://img.shields.io/github/actions/workflow/status/metasequoiaime/msime/ci-platforms.yml?branch=develop&label=Platform%20CI&logo=githubactions&logoColor=white)](https://github.com/metasequoiaime/msime/actions/workflows/ci-platforms.yml?query=branch%3Adevelop)
[![CodeQL](https://img.shields.io/github/actions/workflow/status/metasequoiaime/msime/codeql.yml?branch=develop&event=schedule&label=CodeQL&logo=github)](https://github.com/metasequoiaime/msime/actions/workflows/codeql.yml)
[![Cargo audit](https://img.shields.io/github/actions/workflow/status/metasequoiaime/msime/cargo-audit.yml?branch=develop&event=schedule&label=cargo%20audit&logo=rust)](https://github.com/metasequoiaime/msime/actions/workflows/cargo-audit.yml)

[![Android version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Fandroid%2Fversion.txt&query=%24&prefix=v&label=Android&logo=android&logoColor=white&color=34A853)](../../platforms/android/README.md)
[![iOS version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Fios%2Fversion.txt&query=%24&prefix=v&label=iOS&logo=apple&logoColor=white&color=147EFB)](../../platforms/ios/README.md)
[![macOS version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Fmacos%2Fversion.txt&query=%24&prefix=v&label=macOS&logo=apple&logoColor=white&color=000000)](../../platforms/macos/README.md)
[![Linux version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Flinux%2Fversion.txt&query=%24&prefix=v&label=Linux&logo=linux&logoColor=white&color=D97706)](../../platforms/linux/README.md)
[![Windows version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Fwindows%2Fversion.txt&query=%24&prefix=v&label=Windows&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0wIDBoMTEuMzc3djExLjM3Mkgwem0xMi42MjMgMEgyNHYxMS4zNzJIMTIuNjIzek0wIDEyLjYyM2gxMS4zNzdWMjRIMHptMTIuNjIzIDBIMjRWMjRIMTIuNjIzeiIvPjwvc3ZnPg%3D%3D&logoColor=white&color=0078D4)](../../platforms/windows/README.md)
[![HarmonyOS version](https://img.shields.io/badge/dynamic/yaml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmetasequoiaime%2Fmsime%2Fdevelop%2Fplatforms%2Fharmony%2Fversion.txt&query=%24&prefix=v&label=HarmonyOS&logo=harmonyos&logoColor=white&color=CF0A2C)](../../platforms/harmony/README.md)
[![npm @msime/web-engine](https://img.shields.io/npm/v/@msime/web-engine?label=%40msime%2Fweb-engine&logo=npm)](https://www.npmjs.com/package/@msime/web-engine)
[![crates.io msime-engine](https://img.shields.io/crates/v/msime-engine?label=msime-engine&logo=rust)](https://crates.io/crates/msime-engine)

[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](../../LICENSE)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange?logo=rust)](../../Cargo.toml)
[![Website](https://img.shields.io/badge/website-msime.app-2E7D32)](https://msime.app/)
[![GitHub stars](https://img.shields.io/github/stars/metasequoiaime/msime?style=flat&logo=github)](https://github.com/metasequoiaime/msime/stargazers)
[![Contributors](https://img.shields.io/github/contributors/metasequoiaime/msime)](https://github.com/metasequoiaime/msime/graphs/contributors)
[![Last commit](https://img.shields.io/github/last-commit/metasequoiaime/msime/develop)](https://github.com/metasequoiaime/msime/commits/develop)
<a href="https://depot.dev/?utm_source=metasequoiaime"><img src="https://depot.dev/badges/built-with-depot.svg" alt="Built with Depot" height="20"></a>

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=metasequoiaime/msime&type=Date)](https://star-history.com/#metasequoiaime/msime&Date)

## 模組邊界

- `crates/client-base`、`crates/client-core`、`crates/client-services`、`crates/client-providers`：與宿主無關的用戶端業務，依賴方向是 base → core → services 與 core → providers，四個都不依賴 Tauri、UI 或平台宿主。client-base 是儲存、檔案鎖、文字判定、日曆和簡繁轉換；client-core 是本機設定與版本、主題、標點、剪貼簿、翻譯、雲端候選與雲端剪貼簿協定、固定資源分代安裝；client-services 是帳號與工作階段、社群、使用統計、公告、檢查更新、診斷、剪貼簿中繼、皮膚、外掛、詞庫、常用語和打字統計；client-providers 是語音、服務憑證、AI 和按需下載的資源包。
- `crates/input-runtime`：工作階段編排、焦點取消、候選分頁和帶代次的選擇；不複製 engine 的組詞狀態機。
- `crates/engine`（`msime-engine`）：純 Rust 輸入引擎——組合狀態、各輸入方案、詞庫查詢與學習重播；由原 C++ MSIME-Engine 移植而來。
- `crates/host-api`：版本化 C 介面、綁定執行緒的工作階段控制代碼和明確的回應釋放，六個平台的宿主都連結它。
- `packages/ui`、`apps/desktop`：共用 React 設定頁與 Tauri 承載層，目錄名稱沿用 desktop。React 設定頁是 macOS（Tauri 設定應用程式）、Linux（Tauri 設定應用程式）與 HarmonyOS（打包成 rawfile 由 ArkWeb 載入）的設定介面；Tauri 另在三個桌面平台上承載表情與符號、手寫、螢幕鍵盤、語音、雲端詞庫、雲端剪貼簿等共用面板。Windows、iOS 與 Android 發布套件的設定是原生介面：iOS 與 Android 發布套件不含這些 React 頁面；Windows 的系統匣、開始功能表和不帶面板的請求都開啟原生設定程式 `msime-client-settings.exe`，React 設定頁仍隨 Tauri 的 `MSIME.exe` 安裝，直接執行 `MSIME.exe` 看得到，但沒有設定入口指向它。歸屬見[架構文件](../ARCHITECTURE.md#设置界面按平台归属)。Rust 入口按 `platform/{android,ios,linux,macos,windows,desktop}`、`shared/`、`tests/` 分層。**Tauri 是公用元件，不是任何平台的產品本體**：最終安裝、啟動、被系統識別為輸入法的，始終是 `platforms/<os>` 下的原生宿主，Tauri/React 只在用到它的平台上由它承載。
- `shared/apple/`：macOS 與 iOS 共用的 Foundation / Objective-C++ 橋接，不包含系統輸入法入口。
- `platforms/`：各系統入口和適配層。Android、iOS、macOS、Linux、Windows 與 HarmonyOS 六個平台各自維護宿主邊界，每個宿主都以該系統的原生方式被識別為輸入法：Android 的輸入法服務、iOS 的鍵盤延伸功能、macOS 的 InputMethodKit bundle、Linux 的 IBus 與 Fcitx5 入口、Windows 的 TSF DLL 與 Server、HarmonyOS 的 InputMethodExtensionAbility。共用輸入演算法和組合狀態由 `crates/engine` 持有，宿主不複製。

## 平台目錄與宿主形態

| 平台 | 入口目錄 | 宿主形態與整合方式 |
| --- | --- | --- |
| [Android](../../platforms/android/README.md) | `platforms/android/` | 輸入法服務 `app.msime.android.MSIMEInputService` 跑在 `:ime` 獨立行程，Java 宿主經 `platforms/android/native/client_jni.cpp` 呼叫 host-api；發布的 APK（`build-apk.sh`）用原生 Java 設定介面，不含 Tauri/React；帶 React 設定的 Tauri 合併打包（`build-client-apk.sh`）與輸入法同一套件、不同行程、共用私有 files/bootstrap/state，只用於開發驗證，不發布；手寫走 ML Kit Digital Ink |
| [iOS](../../platforms/ios/README.md) | `platforms/ios/` | XcodeGen 從 `project.yml` 產生的原生 App 內嵌鍵盤延伸功能 `MSIMEKeyboardExtension`，兩者透過 App Group `group.app.msime.ios` 共用狀態；Swift 鍵盤直接呼叫 host-api，手寫走 ML Kit Digital Ink |
| [macOS](../../platforms/macos/README.md) | `platforms/macos/` | InputMethodKit bundle（產物名稱 `水杉输入法.app`，bundle id `app.msime.inputmethod.MetasequoiaIME`），Swift 後端編成 `MSIMEBackend.dylib` 隨 bundle 散布，Sparkle 負責自動更新 |
| [Linux](../../platforms/linux/README.md) | `platforms/linux/` | IBus 與 Fcitx5 是兩個並列的系統入口，連結同一份 host-api ABI；線上聯想、語音、剪貼簿等能力由獨立的 provider 行程加 systemd 使用者單元承載；CPack 產出 TGZ 與 DEB |
| [Windows](../../platforms/windows/README.md) | `platforms/windows/`、`platforms/windows/tsf/` | 行程內 TSF DLL（`MetasequoiaImeTsf`）與行程外 `MetasequoiaImeServer` 經具名管道通訊，另有 watchdog 與執行設定準備工具；候選視窗、浮動工具列與系統匣選單由 Direct2D/DirectWrite 的 `msimeui` 繪製；Inno Setup 安裝程式註冊 TIP 並隨附詞庫 |
| [HarmonyOS](../../platforms/harmony/README.md) | `platforms/harmony/` | ArkTS 的 `KeyboardExtensionAbility`（`module.json5` 宣告 `type: "inputMethod"`）承載鍵盤，經 NAPI 模組 `libmsimeclient.so` 呼叫 host-api；設定頁是內嵌打包進 `entry/src/main/resources/rawfile/settings/index.html` 的同一套共用 React 頁面 |

共用函式庫可以載入不同的宿主行程；不要求啟動 Tauri 才能輸入。六個平台一致：產品形態是 `platforms/<os>` 的原生宿主，Tauri 只提供跨平台共用的功能與介面，不單獨作為產品啟動。跨行程的設定變更走明確的持久化與通知機制，不依賴行程內的共享記憶體。

## CI 與發布

`develop` 上的基礎檢查由 `Core CI` 負責，實際執行的是 actionlint 的 workflow 驗證和相依性審查；儲存庫的文字契約是 `scripts/` 下的那批 `test-*.py`，由 `scripts/run-checks.sh` 依檔名自動探索並執行，`Core CI` 的 contracts job 在 Linux 上跑它，`scripts/verify-local.sh` 在本機跑同一份指令稿；缺少所需工具或參考儲存庫的檢查在 CI 上印出 skipped 並通過。iOS 與 macOS 的編譯與測試由各自的原生宿主 workflow 負責。Android、Linux、HarmonyOS 與 Windows 由 Native Platform CI 在對應平台或共用層有變更時執行，未變更時明確略過；這一層涵蓋宿主契約、JVM 冒煙測試、容器建置和交叉編譯。裝置層級的輸入驗收由各平台 README 記錄的裝置測試套件和手動步驟承擔，不由 CI 代替。各 workflow 的執行狀態見 [GitHub Actions](https://github.com/metasequoiaime/msime/actions)；develop 合併頻繁，排隊中的執行常被後來的推送取消，看結果時以最近一次跑完且未取消的執行為準。

六個平台的發布完全獨立：每個平台有自己的手動 release workflow、版本檔案、並行群組、建置產物和 GitHub Release tag。發布 workflow 只建立 GitHub Release，不自動上傳到應用程式商店，也不使用簽章憑證。

| 平台 | 發布 workflow | 版本檔案 | tag |
| --- | --- | --- | --- |
| Android | [![Android Release](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml) | `platforms/android/version.txt` | `android-vX.Y.Z` |
| iOS | [![iOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml) | `platforms/ios/version.txt` | `ios-vX.Y.Z` |
| macOS | [![macOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml) | `platforms/macos/version.txt` | `macos-vX.Y.Z` |
| Linux | [![Linux Release](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml) | `platforms/linux/version.txt` | `linux-vX.Y.Z` |
| Windows | [![Windows Release](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml) | `platforms/windows/version.txt` | `windows-vX.Y.Z` |
| HarmonyOS | [![HarmonyOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml) | `platforms/harmony/version.txt` | `harmony-vX.Y.Z` |

## 開發

貢獻程式碼前請閱讀 [架構說明](../ARCHITECTURE.md)、[貢獻指南](../../.github/CONTRIBUTING.md)、[安全政策](../../.github/SECURITY.md)、[網路請求與資料流向](../../PRIVACY.md) 和 [行為準則](../../.github/CODE_OF_CONDUCT.md)。準備公開原始碼或平台建置產物時，再閱讀 [開源發布清單](../open-source-release.md) 和[第三方元件清單](../third-party.md)；它們列出第三方聲明、資源授權、敏感檔案檢查和驗證邊界。各平台宿主的建置、測試與安裝細節以對應的 `platforms/<os>/README.md` 為準。製作音效包、音樂包、指令表、特效包、短語表、輔助碼表、單字本或符號集，見[外掛作者指南](../plugins.md)。

跨平台的本機驗證入口是 `bash scripts/verify-local.sh`：`--quick` 只跑編譯階段，是合併前的關卡；不帶參數則跑全量，包含 Rust 測試、fmt、clippy、相依性稽核、前端 lint 與型別檢查、各原生宿主的 ctest 以及整句轉換與逐鍵延遲評測。每個階段把失敗的測試名稱與 `scripts/known-failures.txt` 比對，只因不在清單裡的名稱而判定失敗。`git config core.hooksPath .githooks` 可以把 `--quick` 掛到 pre-push 上。

```sh
cargo test -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --locked
cargo fmt --all --check
cargo clippy -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --all-targets --locked -- -D warnings
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop test
pnpm build
```

桌面建置需要 [Tauri 平台相依套件](https://tauri.app/start/prerequisites/)。不要直接執行 `pnpm tauri build` 或 `pnpm tauri dev`：Tauri 專案只是原生宿主啟動的共用元件，`apps/desktop/scripts/require-platform-build.mjs` 會直接報錯拒絕，建置入口是各平台的 `platforms/<os>`；面向使用者的安裝套件由各平台自己的打包流程產出——macOS 的 CMake bundle、Windows 的 `platforms/windows/installer/msime_setup.iss`、Linux 的 CPack（`-DMSIME_ENABLE_PACKAGING=ON`）、Android 的 `platforms/android/build-apk.sh`、HarmonyOS 的 `hvigorw assembleHap`、iOS 的 Xcode 專案。一般瀏覽器中只顯示無法存取本機設定的提示，不模擬儲存成功。

桌面設定的應用程式識別碼和預設應用程式資料目錄按平台命名：macOS 是 `app.msime.macos`，Windows 是 `app.msime.windows`，Linux 是 `app.msime.linux`。偏好設定儲存在其中的 `preferences.json`。也可用絕對路徑環境變數 `MSIME_CLIENT_STATE_DIR` 指向隔離的開發目錄。macOS 原生宿主讀取同一份設定，並在目前組詞結束後套用更新；多個設定視窗同時儲存時透過 revision 偵測衝突，使用者須明確重新讀取後再決定是否覆寫。

Android 合併打包的建置和裝置測試見 [Android 宿主](../../platforms/android/README.md#tauri--react-共享设置合包)。合併打包只用於開發驗證，發布的 APK 是不含 Tauri 的原生套件；合併打包裡 Tauri 設定與原生 `:ime` 服務同一套件、不同行程，共用私有 files/bootstrap/state，關閉設定視窗不會結束輸入法行程。iOS 的產品宿主是 `platforms/ios` 的原生 App，設定介面是 SwiftUI 原生頁面，它內嵌原生鍵盤延伸功能並透過 App Group 共用狀態；App 裡沒有 WebView，Tauri iOS 專案只在單獨建置公用元件時產出，不作為 App 發布或啟動。簽章與裝置安裝步驟見 [iOS 宿主](../../platforms/ios/README.md)。

共用設定支援 shuangpin_profile：xiaohe（小鶴）、ziranma（自然碼）、shoudao（首道）、microsoft（微軟），以及 custom（自訂雙拼，鍵位表存在 shuangpin_custom_profile）。未設定時按小鶴讀取，未知值拒絕；設定頁在非雙拼方案下停用此選項，但保留已選的值。macOS 和 Linux 的設定頁可以從內建方案起步編輯自訂鍵位、匯入匯出 JSON，鍵位表要通過 Engine 的檢查才能儲存；鍵位表不能用時輸入法按小鶴執行。方案變更沿用組詞結束後再替換 Engine 的規則。

Linux 本機建置、隔離的 D-Bus / IBus 測試和安裝後的首次設定見 [Linux 宿主](../../platforms/linux/README.md)。宿主支援設定檔自動重新讀取，安裝後由隨附安裝的 `msime-linux-setup` 備齊詞庫並準備執行設定；`msime-linux-setup --download` 依 `resources/desktop-dictionary.lock.json` 取回缺少的詞庫，圖形入口在缺少 `runtime-options.json` 時會開啟同一套首次設定頁。

功能對齊以 MSIME-Windows 的完整功能為行為基準：公用業務逐項接入共用層，介面按平台歸屬接入共用 React 頁或該平台的原生設定，Windows 沿用 TSF DLL 與 Server 分行程的結構，但兩者之間的協定只服務本儲存庫同版本的建置，不再相容 MSIME-Windows 的舊設定、共享記憶體、無版本交握和舊語音管道，Android、iOS、macOS、Linux 與 HarmonyOS 依各自的系統能力適配。`scripts/test-reference-*.py` 把這條基準固化成可重跑的關卡——參考實作的出廠設定鍵、四個介面的機器可讀能力清單、changelog 的每條特性、以及 `windows/`、`server/`、`ui/src` 下的每個原始檔，都必須對應到本儲存庫的實作或一條寫明理由的缺席紀錄。

## 在你的輸入法裡使用水杉引擎

水杉的輸入引擎可以單獨嵌進別的輸入法：詞格整句加神經重排、拼寫糾錯、按使用者選擇學習，方案有全拼、雙拼、五筆、日文、韓文、粵拼、注音、越南文、藏文和筆畫。它是開箱即用的引擎庫，不是 Rime 那樣靠方案設定驅動的框架。

- **Rust**：`cargo add msime-engine`（[crates.io](https://crates.io/crates/msime-engine) · [docs.rs](https://docs.rs/msime-engine)），上手見 [crates/engine/README.md](../../crates/engine/README.md) 的「快速上手」。
- **C 和其他能呼叫 C 的語言**（fcitx5、ibus、Windows TSF 等 C/C++ 前端，Swift、Kotlin/JNI、Go、Python、C#）：[GitHub Releases](https://github.com/metasequoiaime/msime/releases?q=msime-engine-capi&expanded=true) 上有 Linux、macOS、Windows 的預先編譯函式庫，附標頭檔 `msime_engine.h` 和 `msime-engine.pc`；上手見 [crates/engine-capi/README.md](../../crates/engine-capi/README.md)。
- **詞庫**：引擎本身不帶詞庫。同一個 Release 裡有全拼、雙拼用的最小資料包。

引擎程式碼是 MPL-2.0，可以嵌進閉源或上架 App Store 的應用程式；詞庫資料保留各自的授權條款，不是 MPL：預設中文詞庫含 GPL-3.0 的 rime-ice 資料，n-gram 計數是 CC-BY-SA-4.0，請依需求另選資料。

完整的介紹、各語言的接入範例和常見問題見官網的[引擎庫頁面](https://msime.app/zh-TW/engine/)。

## 輸入引擎

```sh
cargo test -p msime-engine --locked
```

輸入引擎是 workspace 裡的 `crates/engine`，與其他 crate 一起建置，不需要額外拉取原始碼，也不需要 C++ 工具鏈；SQLite 由 `rusqlite` 的 `bundled` 特性編譯進去。行為基準是從原 C++ Engine 錄下的 `crates/engine/tests/golden/`，錄製方法見 `tools/engine-golden/README.md`。路徑由宿主明確提供，字元輸入是 engine 支援的 ASCII 動作。engine 與執行環境的測試使用真實 engine 而非替身，其中本機 Unicode 模式那組測試案例涵蓋裸數字鍵歸 engine 還是歸候選選擇的判定。

## 原生宿主介面

`cargo build -p msime-host-api --locked` 產出靜態函式庫和動態函式庫。標頭檔為 `crates/host-api/include/msime_client.h`，C 使用範例為 `crates/host-api/native/native_smoke.c`。呼叫鏈為 C 宿主 → host-api → input-runtime → engine，不相依 Tauri 執行環境。

宿主建立工作階段後須明確傳入焦點狀態，將 `handled` 對應為系統吃鍵，將 `commit` 透過系統 API 上屏，將值快照算繪為候選。候選選擇攜帶回傳的 generation 和全域 index。全部工作階段操作都在建立它的執行緒上執行；過期、已銷毀或在錯誤執行緒上使用的控制代碼會回傳錯誤。每個 UTF-8 JSON 回應必須透過 `msime_client_string_free` 釋放一次。逐鍵延遲由 `crates/input-runtime` 的 `rerank_latency` 範例按影格預算測量，`scripts/verify-local.sh` 把它作為一個關卡階段執行。

`msime_client_select_edge(session, generation, index, edge)` 是 ABI 1 的附加介面，轉接器與宿主函式庫須成套更新。方向使用 `MSIME_FIRST_HAN` / `MSIME_LAST_HAN`；共用層核對候選所屬的工作階段、代次和目前頁，engine 擷取首／尾漢字並在成功後清空整個組合。候選沒有漢字時回傳未處理並保留組合，不自動選詞或追加標點；這類有效呼叫仍會更新檢視代次，宿主後續操作必須使用新快照。非法方向和失效身分在狀態推進前即被拒絕。Windows 轉接函式庫的設定鍵路由、無漢字回退與 TSF 回覆編碼都已接進正式環境的 KeyHandler，對應的 `msime-tsf-client-key-router`、`msime-tsf-character-result` 和 `msime-tsf-engine-response` 等測試在 `platforms/windows/tsf` 的 ctest 裡涵蓋這條路徑。

## 固定詞庫資源

`resources/desktop-dictionary.lock.json` 固定 `metasequoiaime/msime-dictionary` 已發布的 `dict-v2.0.41` 的來源、長度和 SHA-256。其中 `msime-mozc_dictionary_oss_README.txt` 與 `msime-mozc_LICENSE.txt` 是日文詞庫的授權條款全文（前者是 IPAdic 與 ICOT 的條款，後者是 Mozc 的三條款 BSD），`msime-scowl_Copyright.txt` 是 `msime-english.db` 裡 SCOWL 詞表的著作權與授權聲明，它們的條款都要求隨資料一同散布，重新打包時不可省略；詳見[第三方元件清單](../third-party.md#日文词库的分发义务)。首次下載約 270 MB。詞庫鎖的 `source_commit` 記錄這批詞庫是本儲存庫哪次提交用 `msime-dict-build` 產出的，其中 `msime-bigram.bin` 與 `msime-trigram.bin` 是整句詞格仲裁用的語言模型表。表缺失時 engine 不報錯，只是整句路徑不加權——候選照常產出，順序變差，所以換詞庫版本時要確認 `crates/engine` 還能讀取新表，並重跑句子轉換評測。開發準備指令：

```sh
cargo run -p msime-client-core --example install_resources -- target/resources
```

安裝程式透過注入的串流讀取資源，限制長度並驗證摘要；全部成功後才發布到以內容識別的目錄。再次使用時檢查快取的位元組；快取損壞時報錯，安裝失敗時不替換舊一代。這裡只準備不可變的發布資源，不啟用現有輸入法，不遷移使用者的學習資料。資源按獨立檔案下載，不整包解壓縮封存檔。

`msime_engine::host::prepare_options` 是工作詞庫準備與學習重播的權威入口，呼叫端必須先驗證資源並暫停相關工作階段。以下探針使用暫存的使用者目錄和快取，驗證已發布詞庫中的 `nihao` 查詢、選詞提交及首／尾漢字選擇：

```sh
cargo run -p msime-engine --example query_dictionary -- <上一步返回的资源目录>
```

整句神經重排模型另有一份 `resources/neural-model.lock.json`，把鍵盤用的小模型和桌面定案時使用的大模型鎖在同一個發布版本。需要單獨準備這兩份模型時執行：

```sh
python3 scripts/fetch_neural_model.py --out target/neural-model
```

下載只接受 HTTPS，先寫入暫存檔，再逐一核對鎖定的長度和 SHA-256 後重新命名發布；已有摘要相符的檔案會略過。發布的安裝套件都不帶桌面模型，使用者開啟「桌面神經聯想」時由設定應用程式按需下載（資源包 `settled-model`）；本機建置和發行版打包仍可以隨附它，這時資源打包優先接受歷史的 `target/settled-model` 目錄，缺少時自動使用這個 `target/neural-model` 目錄裡的桌面模型，隨附的那份優先於下載的。鍵盤模型仍由 `desktop-dictionary.lock.json` 的詞庫資源安裝程式驗證並複製進 EngineResources，兩個鎖都保留作為來源紀錄。

粵拼、注音和筆畫的語言詞庫由 `resources/language-dictionaries.lock.json` 單獨固定，三套詞庫連同各自的授權條款文字是同一個資源包 `language-dictionaries`。macOS 發布套件和 Android 的 full、拼音版發布 APK 只內建打中文所需的核心詞庫：日文詞典那一組（`msime-japanese.dat` 與兩份 Mozc 授權文字）和語言詞庫都不隨附，使用者選擇日文、粵拼、注音或筆畫方案時（Android 是在「新增語言」裡新增這些語言時；拼音版不提供粵拼、注音和筆畫，只會為臨時日語下載日文詞典）才依鎖定檔下載到偏好設定目錄的 `resource-packs/`，下載的檔案與隨附檔案一樣按鎖定的長度和 SHA-256 驗證。Android 的這兩個版本還把非英文離線釋義（`resources/offline-glosses.lock.json`，開啟離線釋義時下載）和本機語音執行函式庫（`resources/voice-runtime-android.lock.json`，開啟「離線辨識」時下載）留給按需下載，啟動時不自動下載任何資源包；從內建這些檔案的舊版升級時，已解壓縮在本機的檔案驗證通過後直接轉成資源包。Windows、Linux、iOS、HarmonyOS、Android 的其他版本和 Tauri 開發用合併打包照舊隨附。macOS 發布套件還把五筆碼表 `msime-wubi.db`（水杉五筆除外，選五筆方案時下載，資源包 `wubi`）和本機語音辨識的 sherpa-onnx 執行函式庫（選用本機辨識時下載，資源包 `voice-runtime-macos`：上游函式庫以本專案的 Developer ID 重新簽署後發布，`resources/voice-runtime-macos.lock.json` 固定其 SHA-256；鎖定檔固定之前仍隨附）留給按需下載。下載網址、鏡像與隱私說明見 [PRIVACY.md](../../PRIVACY.md#资源与更新下载)。

macOS 原生 IMK bundle 的建置、隔離狀態目錄與安裝見 [macOS 宿主](../../platforms/macos/README.md)。`platforms/macos/scripts/install.sh` 用 Developer ID 重新簽章並以原子操作替換到 `~/Library/Input Methods`，失敗時回復；`platforms/macos/scripts/check_input_source.swift` 核查輸入來源的註冊結果。

## 程式碼簽章政策 / Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Windows 正式版目前由 [msime-windows](https://github.com/metasequoiaime/msime-windows) 建置、簽章和發布，簽章適用那邊的 [Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)。本儲存庫的 Windows 安裝套件由 `release-windows.yml` 建置，SignPath 簽章尚未啟用（要等 SignPath Foundation 把本儲存庫加進同一個專案），啟用前發布的是未簽章的安裝套件；啟用後的簽章範圍、角色和流程見本儲存庫的 [Code signing policy](../code-signing-policy.md)。其餘平台依各自平台的機制簽章，不經過 SignPath。隱私說明見 [PRIVACY.md](../../PRIVACY.md)。

## 授權條款

原始碼採用 **GPL-3.0-only**，全文見 [LICENSE](../../LICENSE)。例外是腳本外掛的 SDK（[crates/script-sdk](../../crates/script-sdk/)）、範本（[docs/script-plugin-template](../../docs/script-plugin-template/)）和腳本外掛合集（[plugins/script](../../plugins/script/)）的程式碼採用 MIT，以及輸入引擎（[crates/engine](../../crates/engine/)，`msime-engine`）和 [crates/path-trust](../../crates/path-trust/)（`msime-path-trust`）採用 MPL-2.0（不附 Exhibit B），讓其他輸入法可以嵌入引擎；全文在各自目錄的 `LICENSE`。引擎內嵌的資料表與改編的第三方程式碼保留各自的授權條款，逐個檔案見 [crates/engine/README.md](../../crates/engine/README.md)；引擎執行時讀取的詞庫與模型也按各自的授權條款。對 `crates/engine` 與 `crates/path-trust` 的貢獻以「GPL-3.0-only OR MPL-2.0」授權。整個輸入法產品仍以 GPL-3.0-only 發布。

上游程式碼、詞庫、模型和各平台 SDK 以各自的授權條款和聲明為準，其中 Android 與 iOS 的手寫辨識使用 Google ML Kit，依其服務條款授權，而非開源授權條款。完整對照見[第三方元件清單](../third-party.md)。
