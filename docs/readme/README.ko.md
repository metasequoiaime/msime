# 메타세쿼이아 IME (Metasequoia IME, 水杉输入法)

[简体中文](../../README.md) · [English](README.en.md) · [日本語](README.ja.md) · **한국어** · [繁體中文](README.zh-Hant.md)

> 이 문서는 [간체 중국어 README](../../README.md)의 번역본이며, 간체 중국어판이 기준입니다. 링크된 문서는 대부분 중국어입니다.

### 중국어를 치면, 후보 바로 옆에 영어가.

「영어 뜻 표시」(显示英文释义)를 켜면 오프라인 사전에 뜻이 있는 후보 옆에 영어가 표시됩니다. 컴퓨터에서는 「후보 번역」(候选词翻译)까지 켜면 Ctrl+Enter로 강조된 후보의 영어를 입력합니다. 뜻이 하나면 바로 입력되고, 여러 개면 먼저 목록을 보여 주고 고르게 합니다.

```text
입력         후보 창                Ctrl+Enter로 입력
yusuan   →  预算   budget       →  budget
zhoumo   →  周末   weekend      →  weekend
```

**[메타세쿼이아 IME 다운로드](https://msime.app/download/)** · Windows · macOS · Linux · Android · iOS · HarmonyOS · 무료 오픈 소스(GPL-3.0-only)

- **켜는 방법**: 기본으로 모두 꺼져 있습니다. 설정에서 「영어 뜻 표시」(显示英文释义, Android와 iOS에서는 「오프라인 영어 뜻」离线英文释义)를 켜고, 읽는 법도 보려면 「읽는 법 표시」(显示读音)도 켭니다. 영어 뜻에는 발음 기호, 일본어 뜻에는 로마자, 프랑스어·스페인어·러시아어·독일어·한국어 뜻에는 IPA가 붙습니다. 뜻과 읽는 법은 기기 안의 사전에서 찾으므로 표시할 때 네트워크를 쓰지 않습니다(Android, iOS, Linux에서는 영어 발음 기호 사전과 나머지 여섯 언어의 읽는 법 사전을 설정에서 각각 한 번 내려받습니다). 「후보 번역」도 온라인 번역 서비스를 고르지 않으면 기기 안의 사전만 찾습니다.
- **그 밖에**: 「후보 번역」의 대상 언어는 프랑스어, 일본어, 스페인어, 러시아어, 독일어, 한국어로도 바꿀 수 있습니다(HarmonyOS 휴대폰에는 러시아어가 없습니다). 「단어 외우기」(背单词)에는 중고 입시(中考)부터 GRE까지 단어장 여덟 권이 들어 있고, 간격 반복으로 복습 일정을 잡습니다.
- **플랫폼별 차이**: 읽는 법은 현재 macOS, Linux, Android, iOS, HarmonyOS, Windows 새 버전 미리 보기에서 표시됩니다. Windows 정식판은 [msime-windows](https://github.com/metasequoiaime/msime-windows)가 배포하며 「영어 뜻 표시」라는 별도 스위치가 없고 「후보 번역」(候选词翻译)을 켜면 오프라인 영어 뜻이 표시되며 Ctrl+Enter도 쓸 수 있지만, 발음 기호와 간격 반복 「단어 외우기」 단어장은 아직 없습니다. 모바일 앱은 아직 베타입니다.

**입력기는 여러분이 입력하는 모든 것을 볼 수 있으므로, 이 질문에는 항목별로 확인할 수 있는 답이 있어야 합니다: [어떤 데이터가 기기를 떠나는가](../../PRIVACY.md).** 짧게 말하면, 새로 설치한 기본 설정에서는 입력한 내용을 기기 밖으로 보내는 기능이 없습니다. 클라우드 연상(조합 중인 병음을 Google 입력 도구로 보내는 기능)은 처음 물어볼 때나 설정에서 직접 켜야 하며, 나머지 네트워크 기능은 모두 직접 자격 증명을 입력하거나 서비스를 선택해야만 동작합니다. 이와 별도로 입력 내용을 담지 않는 자체 익명 사용 통계가 하나 있습니다. 여섯 플랫폼 모두 `api.msime.app`에 일일 활성, 세션, 크래시(오류 요약과 디렉터리를 제거한 호출 스택) 기록을 보내며, 기본으로 켜져 있고 설정의 「익명 사용 통계」에서 끌 수 있습니다. 필드, 플랫폼별 차이, 디스크 저장 위치는 [PRIVACY.md](../../PRIVACY.md#使用统计与崩溃上报默认开启可关闭)에 항목별로 나와 있습니다. 저장소는 어떠한 서드파티 통계나 크래시 보고 SDK도 사용하지 않습니다.

메타세쿼이아 IME(MSIME)는 Android, iOS, macOS, Linux, Windows, HarmonyOS를 위한 멀티플랫폼 중국어 입력기입니다. 여섯 플랫폼은 각자 고유한 네이티브 입력기 호스트를 가지며, 모두 동일한 공유 입력 런타임에 연결됩니다. 설정 화면은 플랫폼별로 담당이 나뉘어 macOS, Linux, HarmonyOS는 동일한 공유 React 설정 페이지를, Windows, iOS, Android는 각자의 네이티브 설정 화면을 쓰며, 설정의 비즈니스 규칙은 공유 Rust 계층에 두고 각 플랫폼 화면은 화면만 맡아야 합니다(현재는 일부 규칙이 아직 네이티브 화면 쪽에 적혀 있습니다). React 설정 페이지는 macOS와 Linux에서는 Tauri를 통해 일반 Rust 비즈니스 라이브러리를 호출하고, HarmonyOS에서는 Tauri를 거치지 않고 ArkWeb이 로드하여 ArkTS 네이티브 브리지와 NAPI를 통해 Rust를 호출하며, 입력 알고리즘은 msime-engine이 제공합니다.

프로젝트 홈페이지: [msime.app](https://msime.app/) · [GitHub 저장소](https://github.com/metasequoiaime/msime) · [기여자](https://github.com/metasequoiaime/msime/graphs/contributors) · [문제 보고](https://github.com/metasequoiaime/msime/issues) · [코드 서명 정책 / Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)

> **이름에 관하여**: MSIME는 Metasequoia IME(水杉输入法)의 약칭이며, Microsoft IME와 무관하고 Microsoft와도 아무런 관계가 없습니다. 코드, 패키지 이름, 저장소 이름에 쓰인 `msime`는 모두 이 의미입니다. 설정의 `shuangpin_profile: microsoft`는 「마이크로소프트 쌍병(微软双拼)」 방식으로, Xiaohe(小鹤), Ziranma(自然码), Shoudao(首道)와 나란히 있는 네 가지 키 배치 방식 중 하나이며, 이 키 배치에 익숙한 사용자가 선택할 수 있도록 제공하는 것일 뿐 역시 어떠한 관계도 의미하지 않습니다.

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

## 모듈 경계

- `crates/client-base`, `crates/client-core`, `crates/client-services`, `crates/client-providers`: 호스트와 무관한 클라이언트 비즈니스 로직. 의존 방향은 base → core → services와 core → providers이며, 네 crate 모두 Tauri, UI, 플랫폼 호스트에 의존하지 않습니다. client-base는 저장소, 파일 잠금, 텍스트 판정, 달력, 간체-번체 변환. client-core는 로컬 설정과 에디션, 테마, 문장 부호, 클립보드, 번역, 클라우드 후보와 클라우드 클립보드 프로토콜, 고정 리소스의 세대별 설치. client-services는 계정과 세션, 커뮤니티, 사용 통계, 공지, 업데이트 확인, 진단, 클립보드 릴레이, 스킨, 플러그인, 사전, 상용구, 타이핑 통계. client-providers는 음성, 서비스 자격 증명, AI, 필요할 때 내려받는 리소스 팩.
- `crates/input-runtime`: 세션 오케스트레이션, 포커스 취소, 후보 페이지 나누기, 세대(generation)가 붙은 선택. engine의 단어 조합 상태 머신을 복제하지 않습니다.
- `crates/engine`(`msime-engine`): 순수 Rust 입력 엔진 — 조합 상태, 각 입력 방식, 사전 조회와 학습 재생. 기존 C++ MSIME-Engine에서 이식했습니다.
- `crates/host-api`: 버전이 지정된 C 인터페이스, 스레드에 묶인 세션 핸들, 명시적 응답 해제. 여섯 플랫폼의 호스트가 모두 이를 링크합니다.
- `packages/ui`, `apps/desktop`: 공유 React 설정 페이지와 Tauri 호스팅 계층. 디렉터리 이름은 desktop을 그대로 씁니다. React 설정 페이지는 macOS(Tauri 설정 앱), Linux(Tauri 설정 앱), HarmonyOS(rawfile로 패키징되어 ArkWeb이 로드)의 설정 화면이며, Tauri는 세 데스크톱 플랫폼에서 이모지와 기호, 필기, 화면 키보드, 음성, 클라우드 사전, 클라우드 클립보드 같은 공유 패널도 품습니다. Windows, iOS, Android 릴리스 패키지의 설정은 네이티브 화면입니다. iOS와 Android 릴리스 패키지에는 이 React 페이지가 들어 있지 않습니다. Windows에서는 트레이, 시작 메뉴, 패널을 지정하지 않은 요청이 모두 네이티브 설정 프로그램 `msime-client-settings.exe`를 엽니다. React 설정 페이지는 Tauri의 `MSIME.exe`에 포함된 채 설치되어 `MSIME.exe`를 직접 실행하면 보이지만, 그곳을 가리키는 설정 진입점은 없습니다. 담당 구분은 [아키텍처 문서](../ARCHITECTURE.md#设置界面按平台归属)를 참고하십시오. Rust 진입부는 `platform/{android,ios,linux,macos,windows,desktop}`, `shared/`, `tests/`로 계층이 나뉩니다. **Tauri는 공용 컴포넌트이지 어느 플랫폼의 제품 본체도 아닙니다**: 최종적으로 설치되고 실행되며 시스템에 입력기로 인식되는 것은 항상 `platforms/<os>` 아래의 네이티브 호스트이며, Tauri/React는 그것을 쓰는 플랫폼에서만 그 호스트가 품습니다.
- `shared/apple/`: macOS와 iOS가 함께 쓰는 Foundation / Objective-C++ 브리지이며, 시스템 입력기 진입점은 포함하지 않습니다.
- `platforms/`: 각 시스템의 진입점과 어댑터 계층. Android, iOS, macOS, Linux, Windows, HarmonyOS 여섯 플랫폼은 각자 호스트 경계를 관리하며, 각 호스트는 해당 시스템의 네이티브 방식으로 입력기로 인식됩니다: Android의 입력기 서비스, iOS의 키보드 확장, macOS의 InputMethodKit bundle, Linux의 IBus와 Fcitx5 진입점, Windows의 TSF DLL과 Server, HarmonyOS의 InputMethodExtensionAbility. 공유 입력 알고리즘과 조합 상태는 `crates/engine`이 보유하며, 호스트는 이를 복제하지 않습니다.

## 플랫폼 디렉터리와 호스트 형태

| 플랫폼 | 진입 디렉터리 | 호스트 형태와 통합 방식 |
| --- | --- | --- |
| [Android](../../platforms/android/README.md) | `platforms/android/` | 입력기 서비스 `app.msime.android.MSIMEInputService`는 `:ime` 독립 프로세스에서 실행되며, Java 호스트는 `platforms/android/native/client_jni.cpp`를 거쳐 host-api를 호출합니다. 릴리스되는 APK(`build-apk.sh`)는 네이티브 Java 설정 화면을 쓰며 Tauri/React를 포함하지 않습니다. React 설정을 포함한 Tauri 통합 패키지(`build-client-apk.sh`)는 입력기와 같은 패키지의 다른 프로세스로 비공개 files/bootstrap/state를 공유하지만, 개발용이며 릴리스하지 않습니다. 필기 입력은 ML Kit Digital Ink를 사용합니다 |
| [iOS](../../platforms/ios/README.md) | `platforms/ios/` | XcodeGen이 `project.yml`로부터 생성하는 네이티브 App에 키보드 확장 `MSIMEKeyboardExtension`이 내장되며, 둘은 App Group `group.app.msime.ios`를 통해 상태를 공유합니다. Swift 키보드가 host-api를 직접 호출하고, 필기 입력은 ML Kit Digital Ink를 사용합니다 |
| [macOS](../../platforms/macos/README.md) | `platforms/macos/` | InputMethodKit bundle(산출물 이름 `水杉输入法.app`, bundle id `app.msime.inputmethod.MetasequoiaIME`). Swift 백엔드는 `MSIMEBackend.dylib`로 빌드되어 bundle과 함께 배포되며, Sparkle이 자동 업데이트를 담당합니다 |
| [Linux](../../platforms/linux/README.md) | `platforms/linux/` | IBus와 Fcitx5는 나란히 있는 두 시스템 진입점으로, 동일한 host-api ABI를 링크합니다. 온라인 연상, 음성, 클립보드 등의 기능은 독립 provider 프로세스와 systemd 사용자 유닛이 담당합니다. CPack으로 TGZ와 DEB를 만듭니다 |
| [Windows](../../platforms/windows/README.md) | `platforms/windows/`, `platforms/windows/tsf/` | 프로세스 내 TSF DLL(`MetasequoiaImeTsf`)과 프로세스 외 `MetasequoiaImeServer`가 명명된 파이프로 통신하며, 그 밖에 watchdog과 실행 설정 준비 도구가 있습니다. 후보 창, 플로팅 툴바, 트레이 메뉴는 Direct2D/DirectWrite 기반의 `msimeui`가 그립니다. Inno Setup 설치 프로그램이 TIP을 등록하고 사전을 함께 패키징합니다 |
| [HarmonyOS](../../platforms/harmony/README.md) | `platforms/harmony/` | ArkTS의 `KeyboardExtensionAbility`(`module.json5`에 `type: "inputMethod"` 선언)가 키보드를 담당하며, NAPI 모듈 `libmsimeclient.so`를 거쳐 host-api를 호출합니다. 설정 페이지는 `entry/src/main/resources/rawfile/settings/index.html`에 인라인으로 패키징된 동일한 공유 React 페이지입니다 |

공유 라이브러리는 서로 다른 호스트 프로세스에 로드될 수 있으며, 입력하기 위해 Tauri를 실행할 필요는 없습니다. 여섯 플랫폼 모두 같습니다: 제품 형태는 `platforms/<os>`의 네이티브 호스트이며, Tauri는 플랫폼 간에 공유되는 기능과 화면만 제공할 뿐 독립된 제품으로 실행되지 않습니다. 프로세스 간 설정 변경은 명시적인 영속화와 알림 메커니즘을 거치며, 프로세스 내 공유 메모리에 의존하지 않습니다.

## CI와 릴리스

`develop`의 기본 검사는 `Core CI`가 담당하며, 실제로 실행되는 것은 actionlint의 workflow 검증과 의존성 검토입니다. 저장소의 텍스트 계약은 `scripts/` 아래의 `test-*.py` 묶음으로, `scripts/run-checks.sh`가 파일 이름으로 자동 탐색해 실행합니다. `Core CI`의 contracts job은 Linux에서 이를 실행하고, `scripts/verify-local.sh`는 로컬에서 같은 스크립트를 실행합니다. 필요한 도구나 참조 저장소가 없는 검사는 CI에서 skipped를 출력하고 통과합니다. iOS와 macOS의 빌드와 테스트는 각자의 네이티브 호스트 workflow가 담당합니다. Android, Linux, HarmonyOS, Windows는 해당 플랫폼이나 공유 계층에 변경이 있을 때 Native Platform CI가 실행하며, 변경이 없으면 명시적으로 건너뜁니다. 이 계층은 호스트 계약, JVM 스모크 테스트, 컨테이너 빌드, 크로스 컴파일을 다룹니다. 기기 수준의 입력 인수 검증은 각 플랫폼 README에 기록된 기기 테스트 스위트와 수동 절차가 맡으며, CI가 대신하지 않습니다. 각 workflow의 실행 상태는 [GitHub Actions](https://github.com/metasequoiaime/msime/actions)에서 볼 수 있습니다. develop은 병합이 잦아 대기 중인 실행이 이후의 푸시로 자주 취소되므로, 결과를 볼 때는 가장 최근에 끝까지 실행되고 취소되지 않은 실행을 기준으로 합니다.

여섯 플랫폼의 릴리스는 완전히 독립적입니다: 각 플랫폼마다 자체 수동 release workflow, 버전 파일, 동시성 그룹, 빌드 산출물, GitHub Release tag가 있습니다. 릴리스 workflow는 GitHub Release만 생성하며, 앱 스토어에 자동으로 업로드하거나 서명 자격 증명을 사용하지 않습니다.

| 플랫폼 | 릴리스 workflow | 버전 파일 | tag |
| --- | --- | --- | --- |
| Android | [![Android Release](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml) | `platforms/android/version.txt` | `android-vX.Y.Z` |
| iOS | [![iOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml) | `platforms/ios/version.txt` | `ios-vX.Y.Z` |
| macOS | [![macOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml) | `platforms/macos/version.txt` | `macos-vX.Y.Z` |
| Linux | [![Linux Release](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml) | `platforms/linux/version.txt` | `linux-vX.Y.Z` |
| Windows | [![Windows Release](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml) | `platforms/windows/version.txt` | `windows-vX.Y.Z` |
| HarmonyOS | [![HarmonyOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml) | `platforms/harmony/version.txt` | `harmony-vX.Y.Z` |

## 개발

코드를 기여하기 전에 [아키텍처 설명](../ARCHITECTURE.md), [기여 가이드](../../.github/CONTRIBUTING.md), [보안 정책](../../.github/SECURITY.md), [네트워크 요청과 데이터 흐름](../../PRIVACY.md), [행동 강령](../../.github/CODE_OF_CONDUCT.md)을 읽어 주십시오. 소스 코드나 플랫폼 빌드 산출물을 공개할 준비를 할 때는 [오픈소스 릴리스 체크리스트](../open-source-release.md)와 [서드파티 컴포넌트 목록](../third-party.md)도 읽어 주십시오. 이 문서들에는 서드파티 고지, 리소스 라이선스, 민감한 파일 검사, 검증 범위가 정리되어 있습니다. 각 플랫폼 호스트의 빌드, 테스트, 설치 세부 사항은 해당 `platforms/<os>/README.md`를 기준으로 합니다. 효과음 팩, 음악 팩, 명령표, 특수 효과 팩, 문구표, 보조 코드표, 단어장, 기호 세트를 만들려면 [플러그인 작성자 가이드](../plugins.md)를 참고하십시오.

크로스 플랫폼 로컬 검증 진입점은 `bash scripts/verify-local.sh`입니다. `--quick`은 빌드 단계만 실행하며 병합 전 게이트입니다. 인자 없이 실행하면 전체를 실행하며, Rust 테스트, fmt, clippy, 의존성 감사, 프런트엔드 lint와 타입 검사, 각 네이티브 호스트의 ctest, 그리고 문장 단위 변환과 키 입력별 지연 평가가 포함됩니다. 각 단계는 실패한 테스트 이름을 `scripts/known-failures.txt`와 대조해, 목록에 없는 이름에 대해서만 실패합니다. `git config core.hooksPath .githooks`로 `--quick`을 pre-push에 걸 수 있습니다.

```sh
cargo test -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --locked
cargo fmt --all --check
cargo clippy -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --all-targets --locked -- -D warnings
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop test
pnpm build
```

데스크톱 빌드에는 [Tauri 플랫폼 의존성](https://tauri.app/start/prerequisites/)이 필요합니다. `pnpm tauri build`나 `pnpm tauri dev`를 직접 실행하지 마십시오. Tauri 프로젝트는 네이티브 호스트가 실행하는 공용 컴포넌트일 뿐이며 `apps/desktop/scripts/require-platform-build.mjs`가 오류로 거부합니다. 빌드 진입점은 각 플랫폼의 `platforms/<os>`입니다. 사용자용 설치 패키지는 각 플랫폼 고유의 패키징 체인이 만듭니다 — macOS의 CMake bundle, Windows의 `platforms/windows/installer/msime_setup.iss`, Linux의 CPack(`-DMSIME_ENABLE_PACKAGING=ON`), Android의 `platforms/android/build-apk.sh`, HarmonyOS의 `hvigorw assembleHap`, iOS의 Xcode 프로젝트. 일반 브라우저에서는 로컬 설정에 접근할 수 없다는 안내만 표시하며, 저장 성공을 흉내 내지 않습니다.

데스크톱 설정의 앱 식별자와 기본 앱 데이터 디렉터리는 플랫폼별로 이름이 정해집니다: macOS는 `app.msime.macos`, Windows는 `app.msime.windows`, Linux는 `app.msime.linux`입니다. 환경설정은 그 안의 `preferences.json`에 저장됩니다. 절대 경로 환경 변수 `MSIME_CLIENT_STATE_DIR`로 격리된 개발 디렉터리를 지정할 수도 있습니다. macOS 네이티브 호스트는 같은 설정을 읽고 현재 단어 조합이 끝난 뒤 업데이트를 적용합니다. 여러 설정 창이 동시에 저장하면 revision으로 충돌을 감지하며, 사용자는 명시적으로 다시 읽은 뒤 덮어쓸지 결정해야 합니다.

Android 통합 패키지 빌드와 기기 테스트는 [Android 호스트](../../platforms/android/README.md#tauri--react-共享设置合包)를 참고하십시오. 통합 패키지는 개발용이며, 릴리스되는 APK는 Tauri를 포함하지 않는 네이티브 패키지입니다. 통합 패키지에서 Tauri 설정과 네이티브 `:ime` 서비스는 같은 패키지의 다른 프로세스로 비공개 files/bootstrap/state를 공유하며, 설정 창을 닫아도 입력기 프로세스는 종료되지 않습니다. iOS의 제품 호스트는 `platforms/ios`의 네이티브 App으로, 설정 화면은 네이티브 SwiftUI 페이지입니다. 네이티브 키보드 확장을 내장하고 App Group을 통해 상태를 공유합니다. App에는 WebView가 없으며, Tauri iOS 프로젝트는 공용 컴포넌트를 따로 빌드할 때만 만들어지고 App으로 배포되거나 실행되지 않습니다. 서명과 기기 설치 절차는 [iOS 호스트](../../platforms/ios/README.md)를 참고하십시오.

공유 설정은 shuangpin_profile로 xiaohe(小鹤), ziranma(自然码), shoudao(首道), microsoft(마이크로소프트), 그리고 custom(사용자 정의 쌍병, 키 표는 shuangpin_custom_profile에 저장)을 지원합니다. 기본값은 Xiaohe로 읽으며, 알 수 없는 값은 거부합니다. 설정 페이지는 쌍병이 아닌 방식에서 이 선택을 비활성화하지만 이미 선택된 값은 유지합니다. macOS와 Linux의 설정 페이지에서는 내장 방식을 출발점으로 사용자 정의 키 배치를 편집하고 JSON으로 가져오거나 내보낼 수 있으며, 키 표는 Engine의 검사를 통과해야만 저장되고 쓸 수 없는 표일 때 입력기는 Xiaohe로 동작합니다. 방식 변경은 단어 조합이 끝난 뒤 Engine을 교체하는 규칙을 그대로 따릅니다.

Linux 로컬 빌드, 격리된 D-Bus / IBus 테스트, 설치 후 최초 설정은 [Linux 호스트](../../platforms/linux/README.md)를 참고하십시오. 호스트는 설정 파일 자동 재로드를 지원하며, 설치 후에는 함께 설치되는 `msime-linux-setup`이 사전을 갖추고 실행 설정을 준비합니다. `msime-linux-setup --download`는 `resources/desktop-dictionary.lock.json`에 따라 누락된 사전을 내려받으며, 그래픽 진입점은 `runtime-options.json`이 없을 때 같은 최초 설정 페이지를 엽니다.

기능 정합은 MSIME-Windows의 전체 기능을 동작 기준선으로 삼습니다: 공통 비즈니스 로직은 항목별로 공유 계층에 연결하고 화면은 플랫폼별 담당에 따라 공유 React 페이지나 해당 플랫폼의 네이티브 설정에 연결하며, Windows는 TSF DLL과 Server가 프로세스를 나누는 구조를 그대로 따르지만, 둘 사이의 프로토콜은 이 저장소의 같은 버전 빌드만을 위한 것으로, MSIME-Windows의 옛 설정, 공유 메모리, 버전 없는 핸드셰이크, 옛 음성 파이프와는 더 이상 호환하지 않습니다. Android, iOS, macOS, Linux, HarmonyOS는 각 시스템의 역량에 맞게 적응합니다. `scripts/test-reference-*.py`는 이 기준선을 다시 실행할 수 있는 게이트로 고정합니다 — 참조 구현의 출고 설정 키, 네 화면의 기계 판독 가능한 기능 목록, changelog의 모든 기능 항목, 그리고 `windows/`, `server/`, `ui/src` 아래의 모든 소스 파일은 반드시 이 저장소의 구현이나 이유를 명시한 부재 기록 하나에 대응되어야 합니다.

## 수삼 엔진을 내 입력기에서 사용하기

수삼의 입력 엔진은 다른 입력기에 내장할 수 있습니다. 단어 격자 기반 문장 변환과 신경망 재순위화, 오타 교정, 사용자 선택 학습을 갖추고 전병음, 쌍병음, 오필, 일본어, 한국어, 광둥어 병음, 주음, 베트남어, 티베트어, 필획 입력을 지원합니다. 바로 쓸 수 있는 엔진 라이브러리이며, Rime처럼 스키마로 구동되는 프레임워크는 아닙니다.

- **Rust**: `cargo add msime-engine`([crates.io](https://crates.io/crates/msime-engine) · [docs.rs](https://docs.rs/msime-engine)). 빠른 시작은 [crates/engine/README.md](../../crates/engine/README.md)의 「快速上手」 절에 있습니다.
- **C와 C를 호출할 수 있는 언어**(fcitx5, ibus, Windows TSF 같은 C/C++ 프런트엔드, Swift, Kotlin/JNI, Go, Python, C#): Linux, macOS, Windows용 사전 빌드 라이브러리와 헤더 `msime_engine.h`, `msime-engine.pc`를 [GitHub Releases](https://github.com/metasequoiaime/msime/releases?q=msime-engine-capi&expanded=true)에서 받을 수 있습니다. 빠른 시작은 [crates/engine-capi/README.md](../../crates/engine-capi/README.md)에 있습니다.
- **사전**: 엔진에는 사전이 들어 있지 않습니다. 같은 릴리스에 전병음·쌍병음용 최소 데이터 팩이 있습니다.

엔진 코드는 MPL-2.0이라 비공개 소스나 App Store 앱에도 넣을 수 있습니다. 사전 데이터는 각자의 라이선스를 따르며 MPL이 아닙니다. 기본 중국어 데이터에는 GPL-3.0인 rime-ice 데이터와 CC-BY-SA-4.0인 n-gram 빈도가 들어 있으니 용도에 맞게 데이터를 고르세요.

자세한 소개, 언어별 연동 예시와 자주 묻는 질문은 공식 사이트의 [엔진 라이브러리 페이지](https://msime.app/engine/)(중국어)에 있습니다.

## 입력 엔진

```sh
cargo test -p msime-engine --locked
```

입력 엔진은 workspace의 `crates/engine`으로, 다른 crate와 함께 빌드되며 소스를 추가로 받아오거나 C++ 툴체인을 준비할 필요가 없습니다. SQLite는 `rusqlite`의 `bundled` 기능으로 포함됩니다. 동작 기준은 기존 C++ Engine에서 녹화한 `crates/engine/tests/golden/`이며, 녹화 방법은 `tools/engine-golden/README.md`를 참고하십시오. 경로는 호스트가 명시적으로 제공하며, 문자 입력은 engine이 지원하는 ASCII 동작입니다. engine과 런타임 테스트는 대역(test double)이 아닌 실제 engine을 사용하며, 그중 로컬 Unicode 모드 테스트 케이스 묶음은 숫자 키 단독 입력이 engine에 속하는지 후보 선택에 속하는지의 판정을 다룹니다.

## 네이티브 호스트 인터페이스

`cargo build -p msime-host-api --locked`는 정적 라이브러리와 동적 라이브러리를 생성합니다. 헤더 파일은 `crates/host-api/include/msime_client.h`이고, C 사용 예시는 `crates/host-api/native/native_smoke.c`입니다. 호출 체인은 C 호스트 → host-api → input-runtime → engine이며, Tauri 런타임 의존성이 없습니다.

호스트는 세션을 생성한 뒤 포커스 상태를 명시적으로 전달해야 하며, `handled`는 시스템의 키 소비로, `commit`은 시스템 API를 통한 입력 확정으로, 값 스냅샷은 후보 렌더링으로 매핑해야 합니다. 후보 선택은 반환된 generation과 전역 index를 함께 전달합니다. 모든 세션 작업은 생성한 스레드에서 실행하며, 만료되었거나 파괴되었거나 잘못된 스레드의 핸들은 오류를 반환합니다. 각 UTF-8 JSON 응답은 반드시 `msime_client_string_free`로 한 번 해제해야 합니다. 키 입력별 지연은 `crates/input-runtime`의 `rerank_latency` 예제가 프레임 예산 기준으로 측정하며, `scripts/verify-local.sh`는 이를 하나의 게이트 단계로 실행합니다.

`msime_client_select_edge(session, generation, index, edge)`는 ABI 1의 추가 인터페이스이며, 어댑터와 호스트 라이브러리를 함께 업데이트해야 합니다. 방향은 `MSIME_FIRST_HAN` / `MSIME_LAST_HAN`을 사용합니다. 공유 계층은 후보가 속한 세션, 세대, 현재 페이지를 검증하고, engine은 첫/마지막 한자를 추출해 성공하면 조합 전체를 비웁니다. 후보에 한자가 없으면 미처리를 반환하고 조합을 유지하며, 자동으로 단어를 선택하거나 문장 부호를 덧붙이지 않습니다. 이런 유효한 호출도 뷰 세대는 갱신하므로, 호스트의 이후 작업은 반드시 새 스냅샷을 사용해야 합니다. 잘못된 방향과 무효화된 식별자는 상태가 진행되기 전에 거부됩니다. Windows 어댑터 라이브러리의 설정 키 라우팅, 한자 없음 폴백, TSF 응답 인코딩은 모두 프로덕션 KeyHandler에 연결되어 있으며, 해당하는 `msime-tsf-client-key-router`, `msime-tsf-character-result`, `msime-tsf-engine-response` 등의 테스트가 `platforms/windows/tsf`의 ctest에서 이 경로를 다룹니다.

## 고정 사전 리소스

`resources/desktop-dictionary.lock.json`은 `metasequoiaime/msime-dictionary`에서 발행된 `dict-v2.0.41`의 출처, 길이, SHA-256을 고정합니다. 그중 `msime-mozc_dictionary_oss_README.txt`와 `msime-mozc_LICENSE.txt`는 일본어 사전의 라이선스 전문이고(전자는 IPAdic과 ICOT의 조항, 후자는 Mozc의 3조항 BSD), `msime-scowl_Copyright.txt`는 `msime-english.db`에 들어 있는 SCOWL 단어 목록의 저작권 및 라이선스 고지입니다. 이 조항들은 모두 데이터와 함께 배포할 것을 요구하므로, 다시 패키징할 때 생략해서는 안 됩니다. 자세한 내용은 [서드파티 컴포넌트 목록](../third-party.md#日文词库的分发义务)을 참고하십시오. 최초 다운로드는 약 270 MB입니다. 사전 잠금 파일의 `source_commit`은 이 사전 묶음이 이 저장소의 어느 커밋에서 `msime-dict-build`로 생성되었는지 기록하며, 그중 `msime-bigram.bin`과 `msime-trigram.bin`은 문장 단위 단어 격자(lattice) 판정에 쓰이는 언어 모델 테이블입니다. 테이블이 없어도 engine은 오류를 내지 않고 문장 단위 경로에 가중치를 주지 않을 뿐입니다 — 후보는 그대로 나오지만 순서가 나빠지므로, 사전 버전을 바꿀 때는 `crates/engine`이 새 테이블을 여전히 읽을 수 있는지 확인하고 문장 변환 평가를 다시 실행해야 합니다. 개발 준비 명령:

```sh
cargo run -p msime-client-core --example install_resources -- target/resources
```

설치기는 주입된 스트림으로 리소스를 읽고, 길이를 제한하며 다이제스트를 검증합니다. 모두 성공한 뒤에야 콘텐츠 식별 디렉터리에 게시합니다. 다시 사용할 때는 캐시된 바이트를 검사하며, 손상된 캐시는 오류를 보고하고 실패한 설치는 이전 세대를 대체하지 않습니다. 여기서는 변경 불가능한 릴리스 리소스만 준비하며, 기존 입력기를 활성화하거나 사용자 학습 데이터를 마이그레이션하지 않습니다. 리소스는 개별 파일로 내려받으며, 아카이브 전체를 압축 해제하지 않습니다.

`msime_engine::host::prepare_options`는 작업 사전 준비와 학습 재생의 권위 있는 진입점이며, 호출자는 먼저 리소스를 검증하고 관련 세션을 일시 중지해야 합니다. 다음 프로브는 임시 사용자 디렉터리와 캐시를 사용해, 발행된 사전에서의 `nihao` 조회, 단어 선택 커밋, 첫/마지막 한자 선택을 검증합니다:

```sh
cargo run -p msime-engine --example query_dictionary -- <上一步返回的资源目录>
```

문장 단위 신경망 재순위 모델에는 별도의 `resources/neural-model.lock.json`이 있어, 키보드용 소형 모델과 데스크톱에서 확정 단계에 쓰이는 대형 모델을 같은 릴리스 버전으로 고정합니다. 두 모델을 따로 준비해야 할 때는 다음을 실행합니다:

```sh
python3 scripts/fetch_neural_model.py --out target/neural-model
```

다운로드는 HTTPS만 허용하며, 먼저 임시 파일에 쓴 다음 고정된 길이와 SHA-256을 하나씩 확인한 뒤 이름을 바꿔 게시합니다. 다이제스트가 이미 일치하는 파일은 건너뜁니다. 배포되는 설치 패키지에는 모두 데스크톱 모델이 포함되지 않으며, 사용자가 「데스크톱 신경망 연상」을 켤 때 설정 앱이 필요에 따라 내려받습니다(리소스 팩 `settled-model`). 로컬 빌드와 배포판 패키징에서는 여전히 이를 함께 패키징할 수 있으며, 이때 리소스 패키징은 기존의 `target/settled-model` 디렉터리를 우선 받아들이고, 없으면 이 `target/neural-model` 디렉터리의 데스크톱 모델을 자동으로 사용합니다. 함께 패키징된 모델이 내려받은 모델보다 우선합니다. 키보드 모델은 여전히 `desktop-dictionary.lock.json`의 사전 리소스 설치기가 검증해 EngineResources로 복사하며, 두 잠금 파일은 모두 출처 기록으로 유지됩니다.

광둥어 병음(粤拼), 주음(注音), 획(笔画)의 언어 사전은 `resources/language-dictionaries.lock.json`이 별도로 고정하며, 세 사전과 각각의 라이선스 텍스트가 하나의 리소스 팩 `language-dictionaries`를 이룹니다. macOS 릴리스 패키지와 Android의 full 및 병음판 릴리스 APK에는 중국어 입력에 필요한 핵심 사전만 내장됩니다: 일본어 사전 묶음(`msime-japanese.dat`과 두 개의 Mozc 라이선스 텍스트)과 언어 사전은 함께 패키징되지 않으며, 사용자가 일본어, 광둥어 병음, 주음, 획 방식을 선택할 때(Android에서는 「언어 추가」에서 이 언어들을 추가할 때. 병음판은 광둥어 병음, 주음, 획을 제공하지 않으며 임시 일본어용으로 일본어 사전만 내려받습니다) 비로소 잠금 파일에 따라 환경설정 디렉터리의 `resource-packs/`로 내려받습니다. 내려받은 파일도 함께 패키징된 파일과 마찬가지로 고정된 길이와 SHA-256으로 검증합니다. Android의 이 두 버전은 영어 이외의 오프라인 뜻풀이(`resources/offline-glosses.lock.json`, 오프라인 뜻풀이를 켤 때 내려받음)와 로컬 음성 런타임(`resources/voice-runtime-android.lock.json`, 「오프라인 인식」을 켤 때 내려받음)도 필요 시 다운로드로 남겨 두며, 시작 시 어떤 리소스 팩도 자동으로 내려받지 않습니다. 이 파일들을 내장하던 이전 버전에서 업그레이드할 때는 기기에 이미 압축 해제된 파일이 검증을 통과하면 바로 리소스 팩으로 전환합니다. Windows, Linux, iOS, HarmonyOS, Android의 다른 버전, Tauri 개발용 통합 패키지는 기존처럼 함께 패키징합니다. macOS 릴리스 패키지는 오필 코드표 `msime-wubi.db`(수삼 오필 제외, 오필 방안을 고를 때 내려받음, 리소스 팩 `wubi`)와 기기 내 음성 인식용 sherpa-onnx 런타임(기기 내 인식을 고를 때 내려받음, 리소스 팩 `voice-runtime-macos`: 업스트림 라이브러리를 이 프로젝트의 Developer ID로 다시 서명해 게시하고 `resources/voice-runtime-macos.lock.json`이 SHA-256을 고정함, 이 잠금 파일이 고정하기 전에는 함께 패키징)도 필요 시 다운로드로 남겨 둡니다. 다운로드 주소, 미러, 개인정보 관련 설명은 [PRIVACY.md](../../PRIVACY.md#资源与更新下载)를 참고하십시오.

macOS 네이티브 IMK bundle의 빌드, 격리 상태 디렉터리, 설치는 [macOS 호스트](../../platforms/macos/README.md)를 참고하십시오. `platforms/macos/scripts/install.sh`는 Developer ID로 다시 서명해 `~/Library/Input Methods`에 원자적으로 교체하며, 실패하면 롤백합니다. `platforms/macos/scripts/check_input_source.swift`는 입력 소스 등록 결과를 확인합니다.

## 코드 서명 정책 / Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Windows 정식판은 현재 [msime-windows](https://github.com/metasequoiaime/msime-windows)에서 빌드, 서명, 배포하며, 서명에는 그쪽의 [Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)가 적용됩니다. 이 저장소의 Windows 설치 패키지는 `release-windows.yml`이 빌드하며, SignPath 서명은 아직 활성화되지 않았습니다(SignPath Foundation이 이 저장소를 같은 프로젝트에 추가할 때까지 기다려야 합니다). 활성화 전에는 서명되지 않은 설치 패키지가 배포되며, 활성화 후의 서명 범위, 역할, 절차는 이 저장소의 [Code signing policy](../code-signing-policy.md)를 참고하십시오. 나머지 플랫폼은 각 플랫폼의 메커니즘으로 서명하며 SignPath를 거치지 않습니다. 개인정보 관련 설명은 [PRIVACY.md](../../PRIVACY.md)를 참고하십시오.

## 라이선스

소스 코드는 **GPL-3.0-only**이며, 전문은 [LICENSE](../../LICENSE)를 참고하십시오. 예외로 스크립트 플러그인 SDK([crates/script-sdk](../../crates/script-sdk/)), 템플릿([docs/script-plugin-template](../../docs/script-plugin-template/)), 스크립트 플러그인 모음([plugins/script](../../plugins/script/))의 코드는 MIT이고, 입력 엔진([crates/engine](../../crates/engine/), `msime-engine`)과 [crates/path-trust](../../crates/path-trust/)(`msime-path-trust`)는 다른 입력기에 넣어 쓸 수 있도록 MPL-2.0(Exhibit B 없음)이며, 전문은 각 디렉터리의 `LICENSE`에 있습니다. 엔진에 내장된 데이터 표와 옮겨 온 서드파티 코드는 각자의 라이선스를 따르며 파일별 목록은 [crates/engine/README.md](../../crates/engine/README.md)에 있습니다. 실행 중에 읽는 사전과 모델도 각자의 라이선스를 따릅니다. `crates/engine`과 `crates/path-trust`에 대한 기여는 GPL-3.0-only OR MPL-2.0으로 제공됩니다. 입력기 제품 전체는 GPL-3.0-only로 배포됩니다.

업스트림 코드, 사전, 모델, 각 플랫폼 SDK는 각자의 라이선스와 고지를 따르며, 그중 Android와 iOS의 필기 인식은 Google ML Kit을 사용하고 오픈소스 라이선스가 아닌 해당 서비스 약관에 따라 사용이 허가됩니다. 전체 대조표는 [서드파티 컴포넌트 목록](../third-party.md)을 참고하십시오.
