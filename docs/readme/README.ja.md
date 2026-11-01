# 水杉 IME（Metasequoia IME）

[简体中文](../../README.md) · [English](README.en.md) · **日本語** · [한국어](README.ko.md) · [繁體中文](README.zh-Hant.md)

> これは[簡体字中国語版 README](../../README.md) の翻訳です。内容は簡体字中国語版が正です。リンク先の文書は主に中国語です。

### 中国語を打つと、候補のすぐ横に英語。

「英語の語釈を表示」（显示英文释义）をオンにすると、オフライン辞書に語釈がある候補の横に英語が表示されます。パソコンではさらに「候補の翻訳」（候选词翻译）をオンにすると、Ctrl+Enter でハイライト中の候補の英語を確定できます。語釈が 1 つならそのまま確定し、複数あるときは一覧から選べます。

```text
入力         候補ウィンドウ         Ctrl+Enter で確定
yusuan   →  预算   budget       →  budget
zhoumo   →  周末   weekend      →  weekend
```

**[水杉 IME をダウンロード](https://msime.app/download/)** · Windows · macOS · Linux · Android · iOS · HarmonyOS · 無料・オープンソース（GPL-3.0-only）

- **オンにする方法**：既定ではすべてオフです。設定で「英語の語釈を表示」（显示英文释义。Android と iOS では「オフライン英語語釈」离线英文释义）をオンにし、読みも見たい場合は「読みを表示」（显示读音）もオンにします。英語の語釈には発音記号、日本語の語釈にはローマ字、フランス語・スペイン語・ロシア語・ドイツ語・韓国語の語釈には IPA が付きます。語釈と読みは端末内の辞書から引くため、表示にネットワークは使いません（Android、iOS、Linux では英語の発音記号辞書と、ほかの 6 言語の読み辞書を設定でそれぞれ一度ダウンロードします）。「候補の翻訳」も、オンラインの翻訳サービスを選ばない限り端末内の辞書だけを引きます。
- **ほかにも**：「候補の翻訳」の翻訳先はフランス語、日本語、スペイン語、ロシア語、ドイツ語、韓国語にも切り替えられます（HarmonyOS のスマートフォンにはロシア語がありません）。「単語学習」（背单词）には中考から GRE までの単語帳 8 冊が内蔵され、間隔反復で復習を組みます。
- **プラットフォームによる違い**：読みは現在 macOS、Linux、Android、iOS、HarmonyOS、Windows 新版プレビューで表示されます。Windows の正式版は [msime-windows](https://github.com/metasequoiaime/msime-windows) が配布しており、「英語の語釈を表示」という独立したスイッチはなく、「候補の翻訳」（候选词翻译）をオンにするとオフラインの英語語釈が表示され、Ctrl+Enter も使えます。ただし発音記号と間隔反復の「単語学習」単語帳はまだありません。モバイル版はまだベータ版です。

**入力メソッドはあなたが入力するものをすべて見ることができます。だからこの問いには、一つずつ照合できる答えがあるべきです：[どのデータがデバイスの外に出るのか](../../PRIVACY.md)。**要点だけ言うと、新規インストール時の既定設定では、入力した内容をデバイスの外に送信する機能はありません。クラウド予測（入力中のピンインを Google 入力ツールに送信する機能）は、初回の確認時か設定でユーザーが有効にする必要があり、その他のネットワーク機能は、ユーザー自身が認証情報を入力するかサービスを選択しない限り動作しません。これとは別に、入力内容を含まない独自の匿名利用統計があります。6 つのプラットフォームすべてが `api.msime.app` に日次アクティブ、セッション、クラッシュ（エラーの概要と、ディレクトリを除去したコールスタック）の記録を送信します。これは既定で有効で、設定の「匿名利用統計」でオフにできます。フィールド、プラットフォームごとの違い、ディスク上の保存場所は [PRIVACY.md](../../PRIVACY.md#使用统计与崩溃上报默认开启可关闭) に一つずつ記載しています。リポジトリにはサードパーティの統計 SDK やクラッシュレポート SDK を一切組み込んでいません。

水杉 IME（MSIME）は、Android、iOS、macOS、Linux、Windows、HarmonyOS 向けのマルチプラットフォーム中国語入力メソッドです。6 つのプラットフォームはそれぞれ独自のネイティブ入力メソッドホストを持ち、いずれも同じ共有入力ランタイムに接続しています。設定画面はプラットフォームごとに担当が分かれており、macOS、Linux、HarmonyOS は同じ共有 React 設定ページを使い、Windows、iOS、Android はそれぞれのネイティブ設定画面を使います。設定の業務ルールは共有の Rust 層に置き、各プラットフォームの画面は画面だけを担うべきものとしています（現時点では一部のルールがまだネイティブ画面側に書かれています）。React 設定ページは macOS と Linux では Tauri を介して通常の Rust 業務ライブラリを呼び出し、HarmonyOS では Tauri を経由せず、ArkWeb が読み込んで ArkTS のネイティブブリッジと NAPI を通じて Rust を呼び出します。入力アルゴリズムは msime-engine が提供します。

プロジェクトのリンク：[msime.app](https://msime.app/) · [GitHub リポジトリ](https://github.com/metasequoiaime/msime) · [コントリビューター](https://github.com/metasequoiaime/msime/graphs/contributors) · [問題の報告](https://github.com/metasequoiaime/msime/issues) · [コード署名ポリシー / Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)

> **名称について**：MSIME は Metasequoia IME（水杉 IME）の略称です。Microsoft IME（日本では Windows 標準の日本語入力としておなじみの IME）とは別のソフトウェアであり、Microsoft 社とも一切関係ありません。コード、パッケージ名、リポジトリ名に含まれる `msime` はすべてこの意味です。設定にある `shuangpin_profile: microsoft` は「微軟双拼（Microsoft 双拼）」方式を指し、小鶴、自然碼、首道と並ぶ 4 つのキー配列方式の一つです。このキー配列に慣れたユーザーが選べるように用意しているもので、これも何らかの関係を示すものではありません。

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

## モジュール境界

- `crates/client-base`、`crates/client-core`、`crates/client-services`、`crates/client-providers`：ホストに依存しないクライアントの業務ロジック。依存の向きは base → core → services と core → providers で、4 つとも Tauri、UI、プラットフォームホストには依存しません。client-base はストレージ、ファイルロック、テキスト判定、暦、簡体字から繁体字への変換。client-core はローカル設定とエディション、テーマ、句読点、クリップボード、翻訳、クラウド候補とクラウドクリップボードのプロトコル、固定リソースの世代別インストール。client-services はアカウントとセッション、コミュニティ、利用統計、お知らせ、更新確認、診断、クリップボードリレー、スキン、プラグイン、辞書、定型文、タイピング統計。client-providers は音声、サービスの認証情報、AI、オンデマンドのリソースパック。
- `crates/input-runtime`：セッションのオーケストレーション、フォーカス喪失時のキャンセル、候補のページング、世代付きの選択。engine の変換ステートマシンは複製しません。
- `crates/engine`（`msime-engine`）：純粋な Rust 製の入力エンジン——コンポジション状態、各入力方式、辞書検索、学習のリプレイ。元の C++ 版 MSIME-Engine から移植したものです。
- `crates/host-api`：バージョン付きの C インターフェース、スレッドに紐づいたセッションハンドル、明示的なレスポンス解放。6 つのプラットフォームのホストはすべてこれをリンクします。
- `packages/ui`、`apps/desktop`：共有の React 設定画面と Tauri のホスティング層です。ディレクトリ名は desktop のままです。React 設定ページは macOS（Tauri 設定アプリ）、Linux（Tauri 設定アプリ）、HarmonyOS（rawfile として ArkWeb が読み込む）の設定画面です。Tauri はさらに 3 つのデスクトッププラットフォームで、絵文字と記号、手書き、スクリーンキーボード、音声、クラウド辞書、クラウドクリップボードなどの共有パネルを載せます。Windows、iOS、Android のリリースパッケージの設定はネイティブ画面です。iOS と Android のリリースパッケージにはこれらの React ページが含まれません。Windows ではトレイ、スタートメニュー、パネルを指定しない要求はいずれもネイティブ設定プログラム `msime-client-settings.exe` を開きます。React 設定ページは Tauri の `MSIME.exe` に含まれたままインストールされ、`MSIME.exe` を直接起動すると表示されますが、そこを指す設定の入口はありません。担当の分け方は[アーキテクチャ文書](../ARCHITECTURE.md#设置界面按平台归属)を参照してください。Rust のエントリは `platform/{android,ios,linux,macos,windows,desktop}`、`shared/`、`tests/` に階層分けされています。**Tauri は共通コンポーネントであり、どのプラットフォームの製品本体でもありません**：最終的にインストールされ、起動され、システムから入力メソッドとして認識されるのは常に `platforms/<os>` 配下のネイティブホストであり、Tauri/React はそれを使うプラットフォームでのみそのホストが載せます。
- `shared/apple/`：macOS と iOS で共用する Foundation / Objective-C++ ブリッジです。システム入力メソッドのエントリは含みません。
- `platforms/`：各システムのエントリと適応層です。Android、iOS、macOS、Linux、Windows、HarmonyOS の 6 プラットフォームはそれぞれホスト境界を維持しており、各ホストはそのシステムのネイティブな方法で入力メソッドとして認識されます：Android の入力メソッドサービス、iOS のキーボード拡張、macOS の InputMethodKit bundle、Linux の IBus と Fcitx5 のエントリ、Windows の TSF DLL と Server、HarmonyOS の InputMethodExtensionAbility。共有の入力アルゴリズムとコンポジション状態は `crates/engine` が保持し、ホストは複製しません。

## プラットフォームディレクトリとホスト形態

| プラットフォーム | エントリディレクトリ | ホスト形態と統合方法 |
| --- | --- | --- |
| [Android](../../platforms/android/README.md) | `platforms/android/` | 入力メソッドサービス `app.msime.android.MSIMEInputService` は `:ime` 独立プロセスで動作し、Java ホストは `platforms/android/native/client_jni.cpp` 経由で host-api を呼び出します。リリースされる APK（`build-apk.sh`）はネイティブ Java の設定画面を使い、Tauri/React を含みません。React 設定を含む Tauri 統合パッケージ（`build-client-apk.sh`）は入力メソッドと同じパッケージの別プロセスで、プライベートな files/bootstrap/state を共有しますが、開発用でリリースはしません。手書き入力は ML Kit Digital Ink を使用します |
| [iOS](../../platforms/ios/README.md) | `platforms/ios/` | XcodeGen が `project.yml` から生成するネイティブ App にキーボード拡張 `MSIMEKeyboardExtension` を埋め込み、両者は App Group `group.app.msime.ios` を通じて状態を共有します。Swift のキーボードは host-api を直接呼び出し、手書き入力は ML Kit Digital Ink を使用します |
| [macOS](../../platforms/macos/README.md) | `platforms/macos/` | InputMethodKit bundle（成果物名 `水杉输入法.app`、bundle id `app.msime.inputmethod.MetasequoiaIME`）。Swift バックエンドは `MSIMEBackend.dylib` としてビルドされ bundle とともに配布されます。自動更新は Sparkle が担当します |
| [Linux](../../platforms/linux/README.md) | `platforms/linux/` | IBus と Fcitx5 は並列する 2 つのシステムエントリで、同じ host-api ABI をリンクします。オンライン予測、音声、クリップボードなどの機能は、独立した provider プロセスと systemd ユーザーユニットが担います。CPack で TGZ と DEB を出力します |
| [Windows](../../platforms/windows/README.md) | `platforms/windows/`、`platforms/windows/tsf/` | インプロセスの TSF DLL（`MetasequoiaImeTsf`）とアウトオブプロセスの `MetasequoiaImeServer` が名前付きパイプで通信し、ほかに watchdog と実行設定の準備ツールがあります。候補ウィンドウ、フローティングツールバー、トレイメニューは Direct2D/DirectWrite ベースの `msimeui` が描画します。Inno Setup のインストーラーが TIP を登録し、辞書を同梱します |
| [HarmonyOS](../../platforms/harmony/README.md) | `platforms/harmony/` | ArkTS の `KeyboardExtensionAbility`（`module.json5` で `type: "inputMethod"` を宣言）がキーボードを担い、NAPI モジュール `libmsimeclient.so` 経由で host-api を呼び出します。設定画面は `entry/src/main/resources/rawfile/settings/index.html` にインラインでパッケージされた、同じ共有 React ページです |

共有ライブラリは異なるホストプロセスに読み込むことができ、入力するために Tauri を起動する必要はありません。6 つのプラットフォームで共通して、製品形態は `platforms/<os>` のネイティブホストであり、Tauri はクロスプラットフォームで共有する機能と画面を提供するだけで、単独の製品として起動することはありません。プロセスをまたぐ設定変更は明示的な永続化と通知の仕組みを通じて行い、プロセス内の共有メモリには依存しません。

## CI とリリース

`develop` 上の基本チェックは `Core CI` が担当し、実際に実行するのは actionlint による workflow の検証と依存関係のレビューです。リポジトリのテキスト契約は `scripts/` 配下の一連の `test-*.py` で、`scripts/run-checks.sh` がファイル名から自動検出して実行します。`Core CI` の contracts job は Linux 上でこれを実行し、`scripts/verify-local.sh` はローカルで同じスクリプトを実行します。必要なツールや参照リポジトリがないチェックは、CI 上で skipped と表示して成功扱いになります。iOS と macOS のビルドとテストは、それぞれのネイティブホストの workflow が担当します。Android、Linux、HarmonyOS、Windows は、対応するプラットフォームまたは共有層に変更があったときに Native Platform CI が実行し、変更がなければ明示的にスキップします。この層はホスト契約、JVM スモークテスト、コンテナビルド、クロスコンパイルをカバーします。デバイスレベルの入力受け入れ検証は、各プラットフォームの README に記載されたデバイステストスイートと手動手順が担い、CI では代替しません。各 workflow の実行状況は [GitHub Actions](https://github.com/metasequoiaime/msime/actions) を参照してください。develop へのマージは頻繁で、キュー待ちの実行は後続の push によってキャンセルされることがよくあるため、結果を確認する際は、最後に完了しキャンセルされていない実行を基準にしてください。

6 つのプラットフォームのリリースは完全に独立しています：各プラットフォームに専用の手動 release workflow、バージョンファイル、並行実行グループ、ビルド成果物、GitHub Release tag があります。リリース workflow は GitHub Release を作成するだけで、アプリストアへの自動アップロードや署名用認証情報の使用は行いません。

| プラットフォーム | リリース workflow | バージョンファイル | tag |
| --- | --- | --- | --- |
| Android | [![Android Release](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml) | `platforms/android/version.txt` | `android-vX.Y.Z` |
| iOS | [![iOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml) | `platforms/ios/version.txt` | `ios-vX.Y.Z` |
| macOS | [![macOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml) | `platforms/macos/version.txt` | `macos-vX.Y.Z` |
| Linux | [![Linux Release](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml) | `platforms/linux/version.txt` | `linux-vX.Y.Z` |
| Windows | [![Windows Release](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml) | `platforms/windows/version.txt` | `windows-vX.Y.Z` |
| HarmonyOS | [![HarmonyOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml) | `platforms/harmony/version.txt` | `harmony-vX.Y.Z` |

## 開発

コードを貢献する前に、[アーキテクチャ説明](../ARCHITECTURE.md)、[貢献ガイド](../../.github/CONTRIBUTING.md)、[セキュリティポリシー](../../.github/SECURITY.md)、[ネットワークリクエストとデータの流れ](../../PRIVACY.md)、[行動規範](../../.github/CODE_OF_CONDUCT.md) をお読みください。ソースコードやプラットフォームのビルド成果物を公開する準備をする際は、さらに [オープンソース公開チェックリスト](../open-source-release.md) と[サードパーティコンポーネント一覧](../third-party.md)をお読みください。これらにはサードパーティの通知、リソースのライセンス、機密ファイルのチェック、検証の範囲が記載されています。各プラットフォームホストのビルド、テスト、インストールの詳細は、対応する `platforms/<os>/README.md` の記載が正です。効果音パック、音楽パック、コマンド表、エフェクトパック、フレーズ表、補助コード表、単語帳、記号セットの作成については[プラグイン作者ガイド](../plugins.md)を参照してください。

クロスプラットフォームのローカル検証の入口は `bash scripts/verify-local.sh` です：`--quick` はビルド段階のみを実行し、マージ前のゲートとなります。引数なしでは全項目を実行し、Rust テスト、fmt、clippy、依存関係の監査、フロントエンドの lint と型チェック、各ネイティブホストの ctest、そして文変換と 1 キーごとのレイテンシの評価を含みます。各段階では失敗したテスト名を `scripts/known-failures.txt` と照合し、リストにない名前の場合にのみ失敗とします。`git config core.hooksPath .githooks` で `--quick` を pre-push に組み込めます。

```sh
cargo test -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --locked
cargo fmt --all --check
cargo clippy -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --all-targets --locked -- -D warnings
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop test
pnpm build
```

デスクトップのビルドには [Tauri のプラットフォーム依存関係](https://tauri.app/start/prerequisites/)が必要です。`pnpm tauri build` や `pnpm tauri dev` を直接実行しないでください。Tauri プロジェクトはネイティブホストが起動する共通コンポーネントにすぎず、`apps/desktop/scripts/require-platform-build.mjs` がエラーで拒否します。ビルドの入口は各プラットフォームの `platforms/<os>` です。ユーザー向けのインストールパッケージは、各プラットフォーム独自のパッケージングチェーンが出力します——macOS の CMake bundle、Windows の `platforms/windows/installer/msime_setup.iss`、Linux の CPack（`-DMSIME_ENABLE_PACKAGING=ON`）、Android の `platforms/android/build-apk.sh`、HarmonyOS の `hvigorw assembleHap`、iOS の Xcode プロジェクト。通常のブラウザでは、ローカル設定にアクセスできない旨の案内のみを表示し、保存の成功を装うことはしません。

デスクトップ設定のアプリ識別子と既定のアプリデータディレクトリは、プラットフォームごとに命名されています：macOS は `app.msime.macos`、Windows は `app.msime.windows`、Linux は `app.msime.linux` です。設定はその中の `preferences.json` に保存されます。絶対パスを指定する環境変数 `MSIME_CLIENT_STATE_DIR` で、隔離された開発用ディレクトリを指すこともできます。macOS のネイティブホストは同じ設定を読み込み、現在の変換が終わった後に更新を適用します。複数の設定ウィンドウが同時に保存した場合は revision によって競合を検出し、ユーザーが明示的に再読み込みしたうえで上書きするかどうかを決める必要があります。

Android の統合パッケージのビルドとデバイステストについては [Android ホスト](../../platforms/android/README.md#tauri--react-共享设置合包)を参照してください。統合パッケージは開発用で、リリースされる APK は Tauri を含まないネイティブパッケージです。統合パッケージでは Tauri の設定とネイティブの `:ime` サービスは同じパッケージの別プロセスで、プライベートな files/bootstrap/state を共有し、設定ウィンドウを閉じても入力メソッドのプロセスは終了しません。iOS の製品ホストは `platforms/ios` のネイティブ App で、設定画面はネイティブの SwiftUI ページです。ネイティブのキーボード拡張を埋め込み、App Group を通じて状態を共有します。App には WebView がなく、Tauri の iOS プロジェクトは共通コンポーネントを単独でビルドするときにだけ作られ、App として配布も起動もしません。署名とデバイスへのインストール手順は [iOS ホスト](../../platforms/ios/README.md)を参照してください。

共有設定は shuangpin_profile として xiaohe（小鶴）、ziranma（自然碼）、shoudao（首道）、microsoft（微軟）、および custom（自分で定義する双拼。キー表は shuangpin_custom_profile に保存）をサポートします。省略時は小鶴として読み込み、不明な値は拒否します。設定画面では、双拼以外の方式ではこの選択を無効にしますが、選択済みの値は保持します。macOS と Linux の設定画面では、組み込みの方式を起点に自分のキー配列を編集し、JSON で読み込み・書き出しができます。キー表は Engine の検証を通ったときだけ保存でき、使えないキー表のときは小鶴で動作します。方式の変更は、変換が終わってから Engine を差し替えるという規則に従います。

Linux のローカルビルド、隔離した D-Bus / IBus テスト、インストール後の初回設定については [Linux ホスト](../../platforms/linux/README.md)を参照してください。ホストは設定ファイルの自動再読み込みに対応しており、インストール後は同梱の `msime-linux-setup` が辞書を揃えて実行設定を準備します。`msime-linux-setup --download` は `resources/desktop-dictionary.lock.json` に従って不足している辞書を取得し、グラフィカルなエントリは `runtime-options.json` がない場合に同じ初回設定画面を開きます。

機能の揃え込みは、MSIME-Windows の全機能を動作のベースラインとします：共通の業務ロジックは一項目ずつ共有層に組み込み、画面はプラットフォームごとの担当に従って共有 React ページかそのプラットフォームのネイティブ設定に組み込みます。Windows は TSF DLL と Server を別プロセスに分ける構造を踏襲しますが、両者間のプロトコルは本リポジトリの同一バージョンのビルド間でのみ使用し、MSIME-Windows の旧設定、共有メモリ、バージョンなしのハンドシェイク、旧音声パイプとの互換性はもう維持しません。Android、iOS、macOS、Linux、HarmonyOS はそれぞれのシステムの機能に合わせて適応します。`scripts/test-reference-*.py` はこのベースラインを再実行可能なゲートとして固定しています——参照実装の出荷時設定キー、4 つの画面の機械可読な機能一覧、changelog の各機能項目、そして `windows/`、`server/`、`ui/src` 配下のすべてのソースファイルは、本リポジトリの実装か、理由を明記した欠落記録のいずれかに対応していなければなりません。

## 水杉のエンジンを自分の入力メソッドで使う

水杉の入力エンジンは他の入力メソッドに組み込めます。単語ラティスによる文変換とニューラル再ランキング、入力ミスの補正、ユーザーの選択からの学習を備え、全拼、双拼、五筆、日本語、韓国語、粤拼、注音、ベトナム語、チベット語、筆画に対応します。そのまま使えるエンジンライブラリで、Rime のようなスキーマ駆動のフレームワークではありません。

- **Rust**：`cargo add msime-engine`（[crates.io](https://crates.io/crates/msime-engine) · [docs.rs](https://docs.rs/msime-engine)）。入門は [crates/engine/README.md](../../crates/engine/README.md) の「快速上手」。
- **C と C を呼べる言語**（fcitx5、ibus、Windows TSF などの C/C++ フロントエンド、Swift、Kotlin/JNI、Go、Python、C#）：Linux、macOS、Windows 向けのビルド済みライブラリとヘッダー `msime_engine.h`、`msime-engine.pc` を [GitHub Releases](https://github.com/metasequoiaime/msime/releases?q=msime-engine-capi&expanded=true) で配布しています。入門は [crates/engine-capi/README.md](../../crates/engine-capi/README.md)。
- **辞書**：エンジン自体は辞書を含みません。同じリリースに全拼・双拼用の最小データパックがあります。

エンジンのコードは MPL-2.0 で、クローズドソースや App Store のアプリにも組み込めます。辞書データはそれぞれのライセンスに従い、MPL ではありません。既定の中国語データには GPL-3.0 の rime-ice のデータと CC-BY-SA-4.0 の n-gram 頻度が含まれるため、用途に応じてデータを選んでください。

詳しい紹介、各言語の組み込み例、よくある質問は公式サイトの[エンジンライブラリのページ](https://msime.app/engine/)（中国語）にあります。

## 入力エンジン

```sh
cargo test -p msime-engine --locked
```

入力エンジンは workspace 内の `crates/engine` で、ほかの crate と一緒にビルドされます。追加のソースコードの取得も C++ ツールチェーンも不要で、SQLite は `rusqlite` の `bundled` feature によって組み込まれます。動作の基準は元の C++ Engine から記録した `crates/engine/tests/golden/` で、記録方法は `tools/engine-golden/README.md` を参照してください。パスはホストが明示的に渡し、文字入力は engine がサポートする ASCII アクションです。engine とランタイムのテストはテストダブルではなく実際の engine を使用しており、そのうちローカル Unicode モードのテストケース群は、数字キー単独の入力を engine が処理するのか、候補選択として扱うのかの判定をカバーしています。

## ネイティブホストインターフェース

`cargo build -p msime-host-api --locked` で静的ライブラリと動的ライブラリが生成されます。ヘッダーファイルは `crates/host-api/include/msime_client.h`、C から利用する例は `crates/host-api/native/native_smoke.c` です。呼び出しチェーンは C ホスト → host-api → input-runtime → engine で、Tauri ランタイムへの依存はありません。

ホストはセッションを作成した後、フォーカス状態を明示的に渡し、`handled` をシステム側でのキーの消費に、`commit` をシステム API による確定に、値のスナップショットを候補の描画にマッピングする必要があります。候補の選択には、返された generation とグローバル index を付けます。すべてのセッション操作は作成したスレッドで実行し、期限切れ、破棄済み、またはスレッドが異なるハンドルにはエラーを返します。UTF-8 JSON のレスポンスはそれぞれ `msime_client_string_free` で 1 回だけ解放しなければなりません。1 キーごとのレイテンシは `crates/input-runtime` の `rerank_latency` example がフレーム予算に基づいて計測し、`scripts/verify-local.sh` はそれを 1 つのゲート段階として実行します。

`msime_client_select_edge(session, generation, index, edge)` は ABI 1 の追加インターフェースで、アダプターとホストライブラリはセットで更新する必要があります。方向には `MSIME_FIRST_HAN` / `MSIME_LAST_HAN` を使用します。共有層は候補が属するセッション、世代、現在のページを確認し、engine が先頭／末尾の漢字を取り出して、成功後にコンポジション全体をクリアします。候補に漢字がない場合は未処理を返してコンポジションを保持し、自動で候補を選んだり句読点を追加したりはしません。このような有効な呼び出しでもビューの世代は更新されるため、ホストの後続の操作では新しいスナップショットを使用する必要があります。不正な方向や無効になった識別情報は、状態を進める前に拒否されます。Windows アダプターライブラリの設定キーのルーティング、漢字がない場合のフォールバック、TSF への応答のエンコードはすべて本番の KeyHandler に組み込まれており、対応する `msime-tsf-client-key-router`、`msime-tsf-character-result`、`msime-tsf-engine-response` などのテストが `platforms/windows/tsf` の ctest でこの経路をカバーしています。

## 固定辞書リソース

`resources/desktop-dictionary.lock.json` は、`metasequoiaime/msime-dictionary` の公開済み `dict-v2.0.41` のソース、長さ、SHA-256 を固定します。そのうち `msime-mozc_dictionary_oss_README.txt` と `msime-mozc_LICENSE.txt` は日本語辞書のライセンス全文（前者は IPAdic と ICOT の条項、後者は Mozc の 3 条項 BSD）で、`msime-scowl_Copyright.txt` は `msime-english.db` に含まれる SCOWL 単語リストの著作権とライセンスの表示です。これらの条項はいずれもデータと一緒に配布することを求めているため、再パッケージ時に省略してはいけません。詳しくは[サードパーティコンポーネント一覧](../third-party.md#日文词库的分发义务)を参照してください。初回のダウンロードは約 270 MB です。辞書ロックの `source_commit` は、この辞書一式が本リポジトリのどのコミットで `msime-dict-build` によって生成されたかを記録しており、そのうち `msime-bigram.bin` と `msime-trigram.bin` は文全体の単語ラティスの調停に使う言語モデルの表です。表がない場合でも engine はエラーにならず、文全体の経路で重み付けを行わないだけです——候補は出ますが、順序は悪くなります。そのため辞書のバージョンを変更する際は、`crates/engine` が新しい表を読めることを確認し、文変換の評価を再実行してください。開発準備用のコマンド：

```sh
cargo run -p msime-client-core --example install_resources -- target/resources
```

インストーラーは注入されたストリームからリソースを読み込み、長さを制限してダイジェストを検証します。すべて成功した後にのみ、コンテンツ識別子のディレクトリに公開します。再利用時にはキャッシュのバイトを検査し、破損したキャッシュはエラーとし、失敗したインストールは旧世代を置き換えません。ここでは不変の公開リソースを準備するだけで、既存の入力メソッドを有効化したり、ユーザーの学習データを移行したりはしません。リソースは個別のファイルとしてダウンロードし、アーカイブ全体を展開することはしません。

`msime_engine::host::prepare_options` は作業用辞書の準備と学習のリプレイの正式な入口で、呼び出し側は先にリソースを検証し、関連するセッションを一時停止する必要があります。次のプローブは一時的なユーザーディレクトリとキャッシュを使用して、公開済み辞書での `nihao` の検索、候補選択の確定、先頭／末尾の漢字の選択を検証します：

```sh
cargo run -p msime-engine --example query_dictionary -- <上一步返回的资源目录>
```

文全体のニューラル再ランキングモデルには別途 `resources/neural-model.lock.json` があり、キーボード用の小型モデルと、デスクトップで確定時に使う大型モデルを同じリリースバージョンに固定しています。2 つのモデルを個別に準備する必要がある場合は次を実行します：

```sh
python3 scripts/fetch_neural_model.py --out target/neural-model
```

ダウンロードは HTTPS のみを受け付け、まず一時ファイルに書き込んでから、ロックされた長さと SHA-256 を 1 つずつ照合したうえでリネームして公開します。ダイジェストが一致する既存のファイルはスキップします。公開されるインストールパッケージにはいずれもデスクトップモデルは含まれず、ユーザーが「デスクトップニューラル予測」をオンにしたときに、設定アプリが必要に応じてダウンロードします（リソースパック `settled-model`）。ローカルビルドやディストリビューションのパッケージングでは引き続き同梱することもでき、その場合のリソースのパッケージングは従来の `target/settled-model` ディレクトリを優先し、ない場合はこの `target/neural-model` ディレクトリ内のデスクトップモデルを自動的に使用します。同梱されたものはダウンロードされたものより優先されます。キーボードモデルは引き続き `desktop-dictionary.lock.json` の辞書リソースインストーラーが検証して EngineResources にコピーし、2 つのロックはどちらも出所の記録として保持します。

粤拼、注音、筆画の言語辞書は `resources/language-dictionaries.lock.json` で個別に固定されており、3 つの辞書とそれぞれのライセンス文書は同じリソースパック `language-dictionaries` にまとめられています。macOS のリリースパッケージと、Android の full 版・ピンイン版のリリース APK には、中国語入力に必要なコア辞書のみが内蔵されています：日本語辞書のグループ（`msime-japanese.dat` と 2 つの Mozc ライセンス文書）と言語辞書は同梱されず、ユーザーが日本語、粤拼、注音、筆画の方式を選んだとき（Android では「言語を追加」でこれらの言語を追加したとき。ピンイン版は粤拼、注音、筆画を提供せず、一時的な日本語入力のために日本語辞書をダウンロードするだけです）に初めて、ロックファイルに従って設定ディレクトリの `resource-packs/` にダウンロードされます。ダウンロードしたファイルは同梱ファイルと同様に、ロックされた長さと SHA-256 で検証されます。Android のこの 2 つの版は、英語以外のオフライン語釈（`resources/offline-glosses.lock.json`、オフライン語釈をオンにしたときにダウンロード）とローカル音声ランタイム（`resources/voice-runtime-android.lock.json`、「オフライン認識」をオンにしたときにダウンロード）もオンデマンドのダウンロードに回しており、起動時にリソースパックを自動でダウンロードすることはありません。これらのファイルを内蔵していた旧バージョンからアップグレードした場合、端末上に展開済みのファイルは検証に通ればそのままリソースパックに変換されます。Windows、Linux、iOS、HarmonyOS、Android のその他の版、Tauri の開発用統合パッケージは従来どおり同梱します。macOS のリリースパッケージは、五筆の字根表 `msime-wubi.db`（水杉五筆を除く。五筆を選んだときにダウンロード、リソースパック `wubi`）とオンデバイス音声認識の sherpa-onnx ランタイム（オンデバイス認識を選んだときにダウンロード、リソースパック `voice-runtime-macos`。上流のライブラリを本プロジェクトの Developer ID で再署名して公開し、`resources/voice-runtime-macos.lock.json` が SHA-256 を固定します。ロックが固定するまでは同梱）もオンデマンドのダウンロードに回します。ダウンロード先、ミラー、プライバシーに関する説明は [PRIVACY.md](../../PRIVACY.md#资源与更新下载) を参照してください。

macOS ネイティブの IMK bundle のビルド、隔離された状態ディレクトリ、インストールについては [macOS ホスト](../../platforms/macos/README.md)を参照してください。`platforms/macos/scripts/install.sh` は Developer ID で再署名して `~/Library/Input Methods` にアトミックに置き換え、失敗時はロールバックします。`platforms/macos/scripts/check_input_source.swift` は入力ソースの登録結果を確認します。

## コード署名ポリシー / Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Windows の正式版は現在 [msime-windows](https://github.com/metasequoiaime/msime-windows) でビルド、署名、リリースされており、署名にはそちらの [Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md) が適用されます。本リポジトリの Windows インストーラーは `release-windows.yml` でビルドされますが、SignPath による署名はまだ有効になっていません（SignPath Foundation が本リポジトリを同じプロジェクトに追加するのを待っている状態です）。有効になるまでは未署名のインストーラーを公開します。有効化後の署名の範囲、役割、手順は本リポジトリの [Code signing policy](../code-signing-policy.md) を参照してください。その他のプラットフォームはそれぞれのプラットフォームの仕組みで署名し、SignPath は経由しません。プライバシーに関する説明は [PRIVACY.md](../../PRIVACY.md) を参照してください。

## ライセンス

ソースコードは **GPL-3.0-only** で、全文は [LICENSE](../../LICENSE) を参照してください。例外として、スクリプトプラグインの SDK（[crates/script-sdk](../../crates/script-sdk/)）、テンプレート（[docs/script-plugin-template](../../docs/script-plugin-template/)）、スクリプトプラグイン集（[plugins/script](../../plugins/script/)）のコードは MIT、入力エンジン（[crates/engine](../../crates/engine/)、`msime-engine`）と [crates/path-trust](../../crates/path-trust/)（`msime-path-trust`）は他の入力メソッドに組み込めるよう MPL-2.0（Exhibit B なし）で、全文はそれぞれのディレクトリの `LICENSE` にあります。エンジンに埋め込まれたデータ表と移植したサードパーティのコードはそれぞれのライセンスに従い、ファイルごとの一覧は [crates/engine/README.md](../../crates/engine/README.md) にあります。実行時に読む辞書とモデルもそれぞれのライセンスのままです。`crates/engine` と `crates/path-trust` への貢献は GPL-3.0-only OR MPL-2.0 で提供されます。入力メソッド製品全体は GPL-3.0-only で公開されます。

上流のコード、辞書、モデル、各プラットフォームの SDK は、それぞれのライセンスと通知に従います。そのうち Android と iOS の手書き認識は Google ML Kit を使用しており、オープンソースライセンスではなく、その利用規約に基づいてライセンスされています。完全な対応表は[サードパーティコンポーネント一覧](../third-party.md)を参照してください。
