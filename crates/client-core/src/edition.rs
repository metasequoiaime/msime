//! 产品版本（edition）。
//!
//! 版本表 `shared/contracts/editions.json` 是各版本的单一事实源：每个版本提供哪些输入方案、默认方案是什么、在 `Preferences::default()` 之上叠加哪些默认值、随包带哪些资源和功能。full 是现有产品本身，所有值都等于今天写死在代码里的那个；其他版本是它的收窄。多个版本可以同时安装，彼此完全隔离，因此这里只描述一个版本自己的样子，不涉及版本之间的共享。
//!
//! 版本表在编译期嵌入，结构由本模块的类型解析，跨字段和跨文件的约束（资源组件与锁文件一致、功能依赖的组件、冻结基线等）由 `scripts/test-editions.py` 检查。各平台的身份标识（`platforms` 段）大多由平台构建脚本读取；Rust 进程在运行时要用到的那几段（macOS、Windows 和 Linux）在这里解析，其余平台的段本模块不解析。

use crate::account::AccountPreferenceValue;
use crate::preferences::{InputScheme, TouchKeyboardScheme};
use crate::resources::{ResourceError, ResourceSet};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::OnceLock;

const EDITIONS_JSON: &str = include_str!("../../../shared/contracts/editions.json");

/// 本模块认识的版本表格式版本。
const SCHEMA_VERSION: u32 = 1;

/// 一个产品版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Edition {
    /// 版本 id，例如 `full`、`pinyin`、`wubi`。写入后不再改名。
    pub id: String,
    /// 系统输入法列表和设置应用里显示的产品名。所有版本共用同一个图标。
    pub display_name: DisplayName,
    /// 本版本提供的输入方案，顺序与 `host_surface::compiled_input_schemes()` 一致。不在列表里的方案在本版本中不存在。
    pub input_schemes: Vec<InputScheme>,
    /// 默认方案，也是偏好里的方案不在 `input_schemes` 里时的回退值。
    pub default_scheme: InputScheme,
    /// 叠加在 `Preferences::default()` 之上的版本默认值，用户之后仍可修改。
    pub preference_defaults: PreferenceDefaults,
    /// 随包带的资源。
    pub resources: EditionResources,
    /// 本版本是否提供这些可选功能。
    pub features: EditionFeatures,
    /// 本版本需要的语言词库，取值是 `resources/language-dictionaries.lock.json` 里的条目名。
    pub language_dictionaries: Vec<String>,
    /// 各平台的身份标识中 Rust 进程用得到的那几段。
    #[serde(default)]
    platforms: EditionPlatforms,
}

/// 版本表 `platforms` 里本模块解析的段。其余平台的键由各自的构建脚本读取，这里忽略。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct EditionPlatforms {
    #[serde(default)]
    macos: Option<MacosIdentity>,
    #[serde(default)]
    windows: Option<WindowsIdentity>,
    #[serde(default)]
    linux: Option<LinuxIdentity>,
}

/// 一个版本在 Linux 上的身份标识（版本表 `platforms.linux`）。各版本是各自独立的安装包，可以同时安装：full 装在 `/usr` 下，其他版本装在自己的 `/opt/<package>` 下。C++ 宿主读同一份值生成的 `platforms/linux/src/core/LinuxEdition.h`，脚本由 `platforms/linux/scripts/edition_linux.py` 按版本改写；三边拼出来的目录、socket 和单元名必须一样。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxIdentity {
    /// deb 和 rpm 的包名，也是 systemd 用户单元、图标和 `/usr/bin` 命令名的前缀。
    pub package: String,
    /// 安装前缀：full 是 `/usr`，其他版本是 `/opt/<package>`。
    pub install_prefix: String,
    /// 每个用户的状态目录、运行时目录、下载词库目录和缓存目录的名字（`$XDG_CONFIG_HOME/<client_directory>` 等），也是前缀下 `share`、`lib` 里的子目录名。
    pub client_directory: String,
    /// IBus 组件名和引擎名。
    pub ibus_engine: String,
    /// Fcitx5 插件名和输入法条目名。
    pub fcitx5_addon: String,
    /// 设置应用（Tauri）的 identifier。
    pub tauri_identifier: String,
}

impl LinuxIdentity {
    /// 本版本的一个 systemd 用户单元名，`unit` 是去掉包名前缀的部分，例如 `voice.socket` 得到 `msime-linux-wubi-voice.socket`；full 仍是 `msime-linux-voice.socket`。
    pub fn user_unit(&self, unit: &str) -> String {
        format!("{}-{unit}", self.package)
    }

    /// 本版本的首次配置命令名：full 是 `msime-linux-setup`，其他版本是 `msime-linux-<id>-setup`。
    pub fn setup_program(&self) -> String {
        format!("{}-setup", self.package)
    }

    /// 本版本的设置应用启动命令名：full 是 `msime-linux-settings`，其他版本是 `msime-linux-<id>-settings`。
    pub fn settings_program(&self) -> String {
        format!("{}-settings", self.package)
    }
}

/// 一个版本在 Windows 上的身份标识（版本表 `platforms.windows`）。所有版本两两不同，多个版本可以同时安装，两个版本的 TIP 也可以被同一个应用同时加载。C++ 侧（TSF、Server、看门狗、设置窗口）读同一份值生成的 `shared/contracts/msime_edition.h`，Rust 侧在这里读；两边拼出来的管道、事件和目录名必须一样。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsIdentity {
    /// 输入法注册的语言（十六进制 LANGID，例如 `0x0804`）。
    pub langid: String,
    /// TSF 文本服务的 CLSID，形如 `{E3062E9A-...}`。
    pub clsid: String,
    /// TSF 语言 profile 的 GUID。
    pub profile_guid: String,
    /// TSF 文本服务内部注册的其余 GUID，键名见版本表 schema。
    pub tsf_guids: BTreeMap<String, String>,
    /// Inno Setup 的 AppId。
    pub inno_app_id: String,
    /// 安装器和「已安装的应用」里显示的产品名。
    pub app_name: String,
    /// 注册进系统输入法列表的文本服务名。
    pub text_service_description: String,
    /// Program Files 下的安装目录名，也是安装器默认数据目录 `%LOCALAPPDATA%\<install_dir>` 的名字。
    pub install_dir: String,
    /// HKLM 下记录 `VersionDir`、`ServerPath` 和 `DataDir` 的键。
    pub registry_key: String,
    /// 没有 `DataDir` 时 `%LOCALAPPDATA%` 下的状态目录名。
    pub state_directory: String,
    /// `%LOCALAPPDATA%` 下按 Windows 用户存放匿名账号和使用统计的目录名。
    pub user_data_directory: String,
    /// 覆盖状态目录的环境变量名。
    pub data_dir_environment_variable: String,
    /// 拼在命名管道、命名事件、互斥量和窗口类名后面的后缀；full 是空串。
    pub name_suffix: String,
    /// 看门狗的登录计划任务名。
    pub watchdog_task: String,
    /// msime-host-api 的 DLL 文件名。
    pub host_dll: String,
    /// MSIME.exe（Tauri）的 identifier。
    pub tauri_identifier: String,
    /// 安装包文件名前缀。
    pub installer_base_name: String,
}

impl WindowsIdentity {
    /// 本版本的命名对象名：`base` 加上 [`WindowsIdentity::name_suffix`]。full 的后缀是空串，所以 full 的名字与引入版本之前相同。
    pub fn named(&self, base: &str) -> String {
        format!("{base}{}", self.name_suffix)
    }

    /// 本版本的命名管道路径，例如 `\\.\pipe\FanyImeAuxNamedPipe.wubi`。`base` 是 full 用的管道名（不带 `\\.\pipe\` 前缀），与 `shared/contracts/windows_ipc.h` 一致。
    pub fn pipe_name(&self, base: &str) -> String {
        format!(r"\\.\pipe\{}", self.named(base))
    }
}

/// 一个版本在 macOS 上的身份标识（版本表 `platforms.macos`）。所有版本两两不同，所以多个版本可以同时安装，互不覆盖。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacosIdentity {
    /// InputMethodKit bundle 的 CFBundleIdentifier，也是它的 NSUserDefaults 域。
    pub input_method_bundle_id: String,
    /// 输入法 bundle 的文件名（不含 `.app`）和可执行文件名。
    pub input_method_name: String,
    /// 设置应用的 bundle identifier，也是 Application Support 下状态目录的名字。
    pub settings_bundle_id: String,
    /// 原生账号窗口存访问令牌的钥匙串服务名，刷新令牌在它加 `.refresh` 的服务名下。
    pub keychain_service: String,
    /// Homebrew cask 名。
    pub cask: String,
    /// 发布 DMG 的文件名前缀。
    pub dmg_prefix: String,
}

impl MacosIdentity {
    /// 输入法 bundle 的文件名，例如 `水杉输入法.app`。
    pub fn input_method_bundle_name(&self) -> String {
        format!("{}.app", self.input_method_name)
    }
}

/// 版本的产品名。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayName {
    #[serde(rename = "zh-Hans")]
    pub zh_hans: String,
    pub en: String,
}

/// 版本默认值。`None` 表示沿用 `Preferences::default()`。
///
/// 拒绝未知字段：拼错的键如果被静默忽略，这个版本就会带着错误的默认值发出去。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceDefaults {
    /// 五笔混拼的默认值。
    #[serde(default)]
    pub wubi_mixed_pinyin: Option<bool>,
}

/// 版本随包带的资源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditionResources {
    /// 版本表 `resource_components` 里的组件名。
    pub components: Vec<String>,
}

/// 版本提供的可选功能。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditionFeatures {
    /// 临时日文。
    pub temporary_japanese: bool,
    /// 神经网络整句重排。
    pub neural_keyboard: bool,
    /// 非英文目标语言的离线候选释义（`offline-glosses/zh-<语言>.db`）。这些数据库按中文候选词查释义，所以只有提供中文方案的版本带它们（`scripts/test-editions.py` 检查两者相等）；为 false 的版本各平台都不打包。
    pub offline_glosses: bool,
    /// 手写输入。桌面的 Zinnia 模型（`handwriting-zh_CN.model`）和 Android 的 ML Kit 模型（`zh-Hani-CN`）都只认汉字，所以同样只有提供中文方案的版本有；为 false 的版本不打包、不下载手写模型，也不提供手写面板、手写入口和手写设置页。
    pub handwriting: bool,
}

#[derive(Deserialize)]
struct EditionTable {
    schema_version: u32,
    editions: Vec<Edition>,
}

fn editions() -> &'static [Edition] {
    static EDITIONS: OnceLock<Vec<Edition>> = OnceLock::new();
    EDITIONS.get_or_init(|| {
        let table: EditionTable =
            serde_json::from_str(EDITIONS_JSON).expect("shared/contracts/editions.json is valid");
        assert_eq!(
            table.schema_version, SCHEMA_VERSION,
            "shared/contracts/editions.json schema_version"
        );
        table.editions
    })
}

impl Edition {
    /// full 版本的 id。没有任何版本信息的文档和调用方都按 full 处理。
    pub const FULL_ID: &'static str = "full";

    /// 全部版本，第一个是 full。
    pub fn all() -> &'static [Edition] {
        editions()
    }

    /// 按 id 查找版本；版本表里没有这个 id 时返回 `None`。
    pub fn by_id(id: &str) -> Option<&'static Edition> {
        editions().iter().find(|edition| edition.id == id)
    }

    /// full 版本，即现有产品本身。
    pub fn full() -> &'static Edition {
        Self::by_id(Self::FULL_ID).expect("shared/contracts/editions.json defines the full edition")
    }

    /// HostOptions 文档里记录版本 id 的键。full 的文档不写这个键，所以 full 的文档与引入版本之前逐字节相同，旧版读取方（`HostOptions` 拒绝未知键）照样能读。
    pub const HOST_OPTIONS_KEY: &'static str = "edition";

    /// 是否就是 full 版本。
    pub fn is_full(&self) -> bool {
        self.id == Self::FULL_ID
    }

    /// 本版本资源锁的原文。full 的锁就是 `resources/desktop-dictionary.lock.json` 本身，所以 full 的资源目录、校验和用户词库代次（`ResourceSet::generation`）与引入版本之前完全相同；其他版本的锁由 `scripts/editions.py gen-locks` 按版本表的 `resources.components` 从那份文件机械生成并提交，`scripts/test-editions.py` 检查它们没有漂移。版本表里没有对应锁文件的 id 返回 `None`。
    ///
    /// 逐个列出而不是在构建期生成：`include_str!` 只接受字面路径，加一个版本就在这里加一行。
    pub fn resource_lock(&self) -> Option<&'static str> {
        match self.id.as_str() {
            Self::FULL_ID => Some(include_str!(
                "../../../resources/desktop-dictionary.lock.json"
            )),
            "pinyin" => Some(include_str!("../../../resources/editions/pinyin.lock.json")),
            "wubi" => Some(include_str!("../../../resources/editions/wubi.lock.json")),
            "japanese" => Some(include_str!(
                "../../../resources/editions/japanese.lock.json"
            )),
            "vietnamese" => Some(include_str!(
                "../../../resources/editions/vietnamese.lock.json"
            )),
            "tibetan" => Some(include_str!(
                "../../../resources/editions/tibetan.lock.json"
            )),
            _ => None,
        }
    }

    /// 本版本的资源锁：资源目录必须恰好是这些文件，用户词库代次也按它计算。没有锁文件或锁文件不合法时是 [`ResourceError::InvalidManifest`]。
    pub fn resource_set(&self) -> Result<ResourceSet, ResourceError> {
        let lock = self.resource_lock().ok_or(ResourceError::InvalidManifest)?;
        let set: ResourceSet =
            serde_json::from_str(lock).map_err(|_| ResourceError::InvalidManifest)?;
        set.validate()?;
        Ok(set)
    }

    /// 本版本是否提供这个方案。
    pub fn offers(&self, scheme: InputScheme) -> bool {
        self.input_schemes.contains(&scheme)
    }

    /// 本版本的触屏键盘是否提供这个方案入口：入口背后的输入方案在本版本里时提供。手写不属于任何一个输入方案（识别由平台的手写识别器完成，不经过 Engine 的方案），由版本表的 `features.handwriting` 决定：手写面板写出的是汉字（Android 用的是 ML Kit 的 `zh-Hani-CN` 模型），所以只有提供中文方案的版本有手写，full、拼音版和五笔版都有，日文、越南文和藏文版没有。
    pub fn offers_touch_scheme(&self, scheme: TouchKeyboardScheme) -> bool {
        let input = match scheme {
            TouchKeyboardScheme::Handwriting => return self.features.handwriting,
            TouchKeyboardScheme::Quanpin | TouchKeyboardScheme::NineKey => InputScheme::Quanpin,
            TouchKeyboardScheme::Xiaohe
            | TouchKeyboardScheme::Ziranma
            | TouchKeyboardScheme::Microsoft
            | TouchKeyboardScheme::Shoudao => InputScheme::Shuangpin,
            TouchKeyboardScheme::Wubi => InputScheme::Wubi,
            TouchKeyboardScheme::JapaneseNineKey | TouchKeyboardScheme::Japanese => {
                InputScheme::Japanese
            }
            TouchKeyboardScheme::Korean => InputScheme::Korean,
            TouchKeyboardScheme::Cantonese => InputScheme::Cantonese,
            TouchKeyboardScheme::Zhuyin => InputScheme::Zhuyin,
            TouchKeyboardScheme::Vietnamese => InputScheme::Vietnamese,
            TouchKeyboardScheme::Tibetan => InputScheme::Tibetan,
            TouchKeyboardScheme::Stroke => InputScheme::Stroke,
        };
        self.offers(input)
    }

    /// 本版本在 AI 助手配置文件 `mcpServers` 下登记 `msime-mcp` 用的键：full 仍是 `msime`，其他版本是 `msime-<id>`。多个版本同时安装时，每个版本各登记一条，互不覆盖。
    pub fn mcp_server_name(&self) -> String {
        if self.is_full() {
            "msime".to_owned()
        } else {
            format!("msime-{}", self.id)
        }
    }

    /// HostOptions 文档记录的版本：没有 `edition` 键（或为 null）是 full；键的值不是版本表里的 id 时返回 `None`，调用方应拒绝这份文档，而不是猜成 full。
    pub fn of_host_options(document: &serde_json::Value) -> Option<&'static Edition> {
        match document.get(Self::HOST_OPTIONS_KEY) {
            None | Some(serde_json::Value::Null) => Some(Self::full()),
            Some(serde_json::Value::String(id)) => Self::by_id(id),
            Some(_) => None,
        }
    }

    /// 本版本在 macOS 上的身份标识；版本表里这个版本还没有 macOS 段时为 `None`。
    pub fn macos(&self) -> Option<&MacosIdentity> {
        self.platforms.macos.as_ref()
    }

    /// 本版本在 Windows 上的身份标识；版本表里这个版本还没有 Windows 段时为 `None`。
    pub fn windows(&self) -> Option<&WindowsIdentity> {
        self.platforms.windows.as_ref()
    }

    /// 本版本在 Linux 上的身份标识；版本表里这个版本还没有 Linux 段时为 `None`。
    pub fn linux(&self) -> Option<&LinuxIdentity> {
        self.platforms.linux.as_ref()
    }

    /// 安装包里声明版本的文件名。macOS 的设置应用把它放在 `Contents/Resources/` 下，Windows 的安装包把它放在 Server 目录（`MSIME.exe`、`msime-mcp.exe` 所在的目录）下，Linux 的安装包把它放在前缀的 `bin` 目录（`msime-linux-desktop`、`msime-mcp` 所在的目录）下，内容是 `{"edition": "<id>"}`。full 的包不带这个文件，所以 full 的包与引入版本之前相同。
    pub const PACKAGE_MARKER_FILE: &'static str = "edition.json";

    /// 读取安装包里的版本声明（见 [`Edition::PACKAGE_MARKER_FILE`]）。文件不存在时是 full；文件存在但读不了、不是合法的声明、或声明了版本表里没有的 id 时是错误：一个声明了版本的包不能被当成 full 运行，否则它会去读写 full 的状态目录和输入法。
    pub fn declared_by_package(marker: &Path) -> std::io::Result<&'static Edition> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Marker {
            edition: String,
        }
        let file = match std::fs::File::open(marker) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::full()),
            Err(error) => return Err(error),
        };
        let mut text = String::new();
        file.take(STATE_RECORD_LIMIT).read_to_string(&mut text)?;
        let marker: Marker = serde_json::from_str(&text)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        Self::by_id(&marker.edition)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "unknown edition"))
    }

    /// 本进程所在 macOS 安装包声明的版本。设置应用和 `msime-mcp` 的可执行文件都在 `<App>.app/Contents/MacOS/` 下，声明在 `Contents/Resources/edition.json`（[`Edition::PACKAGE_MARKER_FILE`]）。没有这个文件就是 full：full 的包、开发运行和测试进程都是这样。只在第一次调用时读，之后返回同一个结果。
    ///
    /// 声明坏了（读不了、不是合法的声明、或者是本构建不认识的版本）时是错误而不是 full：一个声明了版本的包不能去停、去改 full 的输入法和状态目录。设置应用启动时就检查它，坏了直接退出。
    pub fn of_macos_bundle() -> Result<&'static Edition, &'static str> {
        static EDITION: OnceLock<Result<&'static Edition, &'static str>> = OnceLock::new();
        *EDITION.get_or_init(|| {
            let executable =
                std::env::current_exe().map_err(|_| "cannot locate the running executable")?;
            let Some(contents) = executable.parent().and_then(Path::parent) else {
                return Ok(Self::full());
            };
            Self::declared_by_package(&contents.join("Resources").join(Self::PACKAGE_MARKER_FILE))
                .map_err(|_| "the package declares an edition this build does not know")
        })
    }

    /// 本进程所在 Windows 安装包声明的版本。`MSIME.exe` 和 `msime-mcp.exe` 安装在 Server 目录里，声明是同目录下的 [`Edition::PACKAGE_MARKER_FILE`]；只有管理员能写 Program Files，普通进程改不了它。没有这个文件就是 full：full 的包、开发运行和测试进程都是这样。只在第一次调用时读，之后返回同一个结果。
    ///
    /// 声明坏了时是错误而不是 full，理由同 [`Edition::of_macos_bundle`]：一个声明了版本的包不能去连 full 的 Server、改 full 的状态目录。
    pub fn of_windows_package() -> Result<&'static Edition, &'static str> {
        static EDITION: OnceLock<Result<&'static Edition, &'static str>> = OnceLock::new();
        *EDITION.get_or_init(Self::declared_beside_executable)
    }

    /// 本进程所在 Linux 安装包声明的版本。`msime-linux-desktop` 和 `msime-mcp` 安装在前缀的 `bin` 目录里，声明是同目录下的 [`Edition::PACKAGE_MARKER_FILE`]；full 装在 `/usr` 下，不带这个文件，其他版本装在 root 才能写的 `/opt/<package>` 下。`/usr/bin` 里指向其他版本的命令是符号链接，`current_exe` 解析到的是链接的目标，所以读到的是目标所在前缀的声明。没有这个文件就是 full：full 的包、开发运行和测试进程都是这样。只在第一次调用时读，之后返回同一个结果。
    ///
    /// 声明坏了时是错误而不是 full，理由同 [`Edition::of_macos_bundle`]：一个声明了版本的包不能去连 full 的 socket、改 full 的状态目录。
    pub fn of_linux_package() -> Result<&'static Edition, &'static str> {
        static EDITION: OnceLock<Result<&'static Edition, &'static str>> = OnceLock::new();
        *EDITION.get_or_init(Self::declared_beside_executable)
    }

    /// 本进程所在 Linux 安装包的版本在 Linux 上的身份标识（见 [`Edition::of_linux_package`]）。
    pub fn linux_package_identity() -> Result<&'static LinuxIdentity, &'static str> {
        Self::of_linux_package()?
            .linux()
            .ok_or("this edition has no Linux identifiers")
    }

    /// 同 [`Edition::linux_package_identity`]，但声明坏了时退回 full 的身份。只给启动时已经检查过声明的进程用（设置应用声明坏了就退出），这样它们取每用户目录名、socket 目录和单元名时不必再处理一个不会发生的错误。
    pub fn linux_package_identity_or_full() -> &'static LinuxIdentity {
        Self::linux_package_identity().unwrap_or_else(|_| {
            Self::full()
                .linux()
                .expect("shared/contracts/editions.json gives full Linux identifiers")
        })
    }

    /// 与可执行文件同目录的版本声明（Windows 的 Server 目录、Linux 前缀的 `bin` 目录）。
    fn declared_beside_executable() -> Result<&'static Edition, &'static str> {
        let executable =
            std::env::current_exe().map_err(|_| "cannot locate the running executable")?;
        let Some(directory) = executable.parent() else {
            return Ok(Self::full());
        };
        Self::declared_by_package(&directory.join(Self::PACKAGE_MARKER_FILE))
            .map_err(|_| "the package declares an edition this build does not know")
    }

    /// 本进程所在 Windows 安装包的版本在 Windows 上的身份标识（见 [`Edition::of_windows_package`]）。
    pub fn windows_package_identity() -> Result<&'static WindowsIdentity, &'static str> {
        Self::of_windows_package()?
            .windows()
            .ok_or("this edition has no Windows identifiers")
    }

    /// 状态目录里记录它属于哪个版本的文件名，内容就是版本 id。
    ///
    /// 版本之间完全隔离，一个状态目录只属于一个版本，而各平台读写偏好的 C ABI 和设置应用的偏好存储只拿到这个目录：偏好文件不见了或要修复时，靠这份记录才知道该用哪个版本的默认偏好（`PreferencesStore::new`）。full 不写这个文件，所以 full 的状态目录与引入版本之前相同。
    pub const STATE_RECORD_FILE: &'static str = "edition";

    /// `state_root` 里记录的版本（见 [`Edition::STATE_RECORD_FILE`]）。没有记录、读不到、或记录的 id 不在版本表里时返回 `None`，调用方按 full 处理：这份记录只决定缺省值，认不出时用 full 的缺省值不会让任何方案超出 HostOptions 文档记录的版本。
    pub fn recorded_in(state_root: &Path) -> Option<&'static Edition> {
        let path = state_root.join(Self::STATE_RECORD_FILE);
        crate::storage::reject_symlink(&path).ok()?;
        let mut text = String::new();
        std::fs::File::open(path)
            .ok()?
            .take(STATE_RECORD_LIMIT)
            .read_to_string(&mut text)
            .ok()?;
        Self::by_id(text.trim())
    }

    /// 把 `state_root` 记成属于本版本：不是 full 时写下 [`Edition::STATE_RECORD_FILE`]，是 full 时删掉可能留着的记录。准备宿主（`prepare_host_configuration_for_edition`）时调用，`state_root` 必须已经存在。记录已经是本版本时不重写。
    pub fn record_in(&self, state_root: &Path) -> std::io::Result<()> {
        let path = state_root.join(Self::STATE_RECORD_FILE);
        crate::storage::reject_symlink(&path)?;
        if self.is_full() {
            return match std::fs::remove_file(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                result => result,
            };
        }
        if Self::recorded_in(state_root).is_some_and(|recorded| recorded.id == self.id) {
            return Ok(());
        }
        let mut temporary = tempfile::NamedTempFile::new_in(state_root)?;
        temporary.write_all(format!("{}\n", self.id).as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// 状态目录里的版本记录最多读这么多字节；版本 id 远比它短。
const STATE_RECORD_LIMIT: u64 = 256;

/// 账号偏好里记录输入方案的键。
const ACCOUNT_SCHEME_KEY: &str = "input.schema";
/// iOS 的九键开关：它说的是 `input.schema` 那个方案用不用九键，只随方案一起同步。
const ACCOUNT_NINE_KEY_KEY: &str = "platform.ios.nine_key";
/// Android 的触屏布局（26 键、九键、手写）。它和 `input.schema` 一起才决定键盘上的那个入口（全拼加九键是全拼 9 键，全拼加手写是手写），所以同样只随方案一起同步：五笔版不同步方案，也就不同步布局，否则 full 那边选的全拼 9 键会把五笔版的键盘换成九键布局。
const ACCOUNT_ANDROID_LAYOUT_KEY: &str = "platform.android.keyboard_layout";
/// 双拼方案的键。不提供双拼的版本里它只是本机的缺省值，不该盖掉账号里别的设备选的方案。
const ACCOUNT_SHUANGPIN_KEY: &str = "input.shuangpin_schema";
/// 五笔版本（86/98）的键，理由同上。
const ACCOUNT_WUBI_KEY: &str = "input.wubi_schema";

/// 账号设置里 `input.schema` 的值对应的方案；不是字符串或不认识的取值返回 `None`。
fn account_scheme(settings: &BTreeMap<String, AccountPreferenceValue>) -> Option<InputScheme> {
    match settings.get(ACCOUNT_SCHEME_KEY) {
        Some(AccountPreferenceValue::String(value)) => {
            serde_json::from_value(serde_json::Value::String(value.clone())).ok()
        }
        _ => None,
    }
}

/// 本版本是否同步 `input.schema`：只有一个方案的版本没有可选的方案，既不上传也不应用它，否则会把 full 等其他版本记在账号里的方案盖掉，或把账号里的方案带进本版本。`None` 是认不出的版本，同样不同步。
fn syncs_account_scheme(edition: Option<&Edition>) -> bool {
    edition.is_some_and(|edition| edition.input_schemes.len() > 1)
}

/// 本版本不同步 `input.schema`，或它的值是本版本不提供的方案时，去掉它和随它的 iOS 九键开关、Android 触屏布局。
fn drop_unsynced_account_scheme(
    edition: Option<&Edition>,
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
) {
    let keeps_scheme = syncs_account_scheme(edition)
        && account_scheme(settings)
            .is_none_or(|scheme| edition.is_some_and(|edition| edition.offers(scheme)));
    if !keeps_scheme {
        settings.remove(ACCOUNT_SCHEME_KEY);
        settings.remove(ACCOUNT_NINE_KEY_KEY);
        settings.remove(ACCOUNT_ANDROID_LAYOUT_KEY);
    }
}

/// 上传前按版本过滤本机整理出的账号设置。各平台把本机偏好换成账号设置之后、合并进账号文档之前调用。
///
/// - 只有一个方案的版本（以及认不出的版本，`edition` 为 `None`）不上传 `input.schema` 和随它的 iOS 九键开关、Android 触屏布局；
/// - 有多个方案的版本只上传本版本提供的方案；
/// - 不提供双拼、五笔的版本不上传双拼方案、五笔版本这两项。
///
/// full 提供全部方案，什么也不去掉。
pub fn filter_uploaded_account_settings(
    edition: Option<&Edition>,
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
) {
    drop_unsynced_account_scheme(edition, settings);
    if !edition.is_some_and(|edition| edition.offers(InputScheme::Shuangpin)) {
        settings.remove(ACCOUNT_SHUANGPIN_KEY);
    }
    if !edition.is_some_and(|edition| edition.offers(InputScheme::Wubi)) {
        settings.remove(ACCOUNT_WUBI_KEY);
    }
}

/// 应用前按版本过滤从账号下载的设置，各平台在解析账号文档之前调用。
///
/// - 只有一个方案的版本（以及认不出的版本）忽略 `input.schema` 和随它的 iOS 九键开关、Android 触屏布局；
/// - 有多个方案的版本把本版本不提供的方案当作账号里没有这一项，本机方案保持不变，文档其余部分照常应用。不认识的取值留给平台代码按原来的规则处理。
///
/// full 提供全部方案，什么也不去掉。
pub fn filter_downloaded_account_settings(
    edition: Option<&Edition>,
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
) {
    drop_unsynced_account_scheme(edition, settings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_surface::compiled_input_schemes;
    use crate::preferences::ChineseScheme;
    use crate::preferences::Preferences;
    use std::collections::BTreeSet;

    #[test]
    fn the_table_parses_and_full_comes_first() {
        let ids: Vec<&str> = Edition::all()
            .iter()
            .map(|edition| edition.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "full",
                "pinyin",
                "wubi",
                "japanese",
                "vietnamese",
                "tibetan"
            ]
        );
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    }

    #[test]
    fn full_matches_the_product_as_it_is_today() {
        let full = Edition::full();
        assert_eq!(full.id, Edition::FULL_ID);
        assert_eq!(full.input_schemes, compiled_input_schemes());
        assert_eq!(full.default_scheme, InputScheme::Quanpin);
        assert_eq!(full.default_scheme, Preferences::default().scheme);
        assert_eq!(full.preference_defaults, PreferenceDefaults::default());
        assert_eq!(
            full.features,
            EditionFeatures {
                temporary_japanese: true,
                neural_keyboard: true,
                offline_glosses: true,
                handwriting: true,
            }
        );
        assert_eq!(full.display_name.zh_hans, "水杉输入法");
        assert_eq!(full.display_name.en, "MSIME");
    }

    #[test]
    fn pinyin_offers_quanpin_and_shuangpin_with_temporary_japanese() {
        let pinyin = Edition::by_id("pinyin").unwrap();
        assert_eq!(
            pinyin.input_schemes,
            [InputScheme::Quanpin, InputScheme::Shuangpin]
        );
        assert_eq!(pinyin.default_scheme, InputScheme::Quanpin);
        assert!(pinyin.features.temporary_japanese);
        assert_eq!(pinyin.display_name.zh_hans, "水杉拼音");
        assert_eq!(pinyin.display_name.en, "MSIME Pinyin");
    }

    #[test]
    fn wubi_offers_only_wubi_with_mixed_pinyin_on_by_default() {
        let wubi = Edition::by_id("wubi").unwrap();
        assert_eq!(wubi.input_schemes, [InputScheme::Wubi]);
        assert_eq!(wubi.default_scheme, InputScheme::Wubi);
        assert_eq!(wubi.preference_defaults.wubi_mixed_pinyin, Some(true));
        assert!(!wubi.features.temporary_japanese);
        assert!(wubi.features.offline_glosses);
        assert!(wubi.features.handwriting);
        assert!(wubi.offers(InputScheme::Wubi));
        assert!(!wubi.offers(InputScheme::Quanpin));
        assert_eq!(wubi.display_name.zh_hans, "水杉五笔");
        assert_eq!(wubi.display_name.en, "MSIME Wubi");
    }

    /// 日文、越南文和藏文各是只有一个方案的版本：默认方案就是它，不带临时日文和键盘神经联想，也没有任何版本默认值。
    #[test]
    fn the_language_editions_offer_one_scheme_each() {
        for (id, scheme, zh_hans, en) in [
            (
                "japanese",
                InputScheme::Japanese,
                "水杉日语",
                "MSIME Japanese",
            ),
            (
                "vietnamese",
                InputScheme::Vietnamese,
                "水杉越南语",
                "MSIME Vietnamese",
            ),
            ("tibetan", InputScheme::Tibetan, "水杉藏文", "MSIME Tibetan"),
        ] {
            let edition = Edition::by_id(id).unwrap();
            assert_eq!(edition.input_schemes, [scheme], "{id}");
            assert_eq!(edition.default_scheme, scheme, "{id}");
            assert_eq!(
                edition.preference_defaults,
                PreferenceDefaults::default(),
                "{id}"
            );
            assert_eq!(
                edition.features,
                EditionFeatures {
                    temporary_japanese: false,
                    neural_keyboard: false,
                    offline_glosses: false,
                    handwriting: false,
                },
                "{id}"
            );
            assert!(edition.language_dictionaries.is_empty(), "{id}");
            assert_eq!(edition.display_name.zh_hans, zh_hans);
            assert_eq!(edition.display_name.en, en);
            assert_eq!(edition.mcp_server_name(), format!("msime-{id}"));
        }
    }

    #[test]
    fn every_edition_defaults_to_a_scheme_it_offers() {
        for edition in Edition::all() {
            assert!(edition.offers(edition.default_scheme), "{}", edition.id);
            assert!(
                edition
                    .input_schemes
                    .iter()
                    .all(|scheme| compiled_input_schemes().contains(scheme)),
                "{}",
                edition.id
            );
        }
    }

    /// 离线释义和手写只认中文：两者都等于版本是否提供中文方案，与 `scripts/test-editions.py` 对版本表的检查是同一条规则。
    #[test]
    fn chinese_only_features_follow_the_chinese_schemes() {
        for edition in Edition::all() {
            let writes_chinese = edition
                .input_schemes
                .iter()
                .any(|scheme| ChineseScheme::of(*scheme).is_some());
            assert_eq!(
                edition.features.offline_glosses, writes_chinese,
                "{}",
                edition.id
            );
            assert_eq!(
                edition.features.handwriting, writes_chinese,
                "{}",
                edition.id
            );
            assert_eq!(
                edition.offers_touch_scheme(TouchKeyboardScheme::Handwriting),
                writes_chinese,
                "{}",
                edition.id
            );
        }
    }

    #[test]
    fn an_unknown_id_is_not_an_edition() {
        assert!(Edition::by_id("unknown").is_none());
        assert!(Edition::by_id("").is_none());
    }

    #[test]
    fn host_options_without_an_edition_are_full() {
        use serde_json::json;
        let of = |document: serde_json::Value| {
            Edition::of_host_options(&document).map(|e| e.id.as_str())
        };
        assert_eq!(of(json!({ "api_version": 1 })), Some("full"));
        assert_eq!(of(json!({ "edition": null })), Some("full"));
        assert_eq!(of(json!({ "edition": "full" })), Some("full"));
        assert_eq!(of(json!({ "edition": "wubi" })), Some("wubi"));
        assert_eq!(of(json!({ "edition": "pinyin" })), Some("pinyin"));
        assert_eq!(of(json!({ "edition": "japanese" })), Some("japanese"));
        assert_eq!(of(json!({ "edition": "vietnamese" })), Some("vietnamese"));
        assert_eq!(of(json!({ "edition": "tibetan" })), Some("tibetan"));
        assert_eq!(of(json!({ "edition": "klingon" })), None);
        assert_eq!(of(json!({ "edition": 3 })), None);
    }

    #[test]
    fn full_keeps_the_mcp_server_name_and_the_others_get_their_own() {
        assert_eq!(Edition::full().mcp_server_name(), "msime");
        assert_eq!(
            Edition::by_id("wubi").unwrap().mcp_server_name(),
            "msime-wubi"
        );
        assert_eq!(
            Edition::by_id("pinyin").unwrap().mcp_server_name(),
            "msime-pinyin"
        );
    }

    #[test]
    fn touch_scheme_entries_follow_the_input_schemes_and_handwriting_needs_a_chinese_scheme() {
        let full = Edition::full();
        assert!(TouchKeyboardScheme::ALL
            .into_iter()
            .all(|scheme| full.offers_touch_scheme(scheme)));
        let wubi = Edition::by_id("wubi").unwrap();
        let offered: Vec<_> = TouchKeyboardScheme::ALL
            .into_iter()
            .filter(|scheme| wubi.offers_touch_scheme(*scheme))
            .collect();
        assert_eq!(
            offered,
            [TouchKeyboardScheme::Wubi, TouchKeyboardScheme::Handwriting]
        );
        let japanese = Edition::by_id("japanese").unwrap();
        let offered: Vec<_> = TouchKeyboardScheme::ALL
            .into_iter()
            .filter(|scheme| japanese.offers_touch_scheme(*scheme))
            .collect();
        assert_eq!(
            offered,
            [
                TouchKeyboardScheme::JapaneseNineKey,
                TouchKeyboardScheme::Japanese
            ]
        );
        // 手写识别器只认汉字，没有中文方案的版本不提供手写。
        for (id, scheme) in [
            ("vietnamese", TouchKeyboardScheme::Vietnamese),
            ("tibetan", TouchKeyboardScheme::Tibetan),
        ] {
            let edition = Edition::by_id(id).unwrap();
            let offered: Vec<_> = TouchKeyboardScheme::ALL
                .into_iter()
                .filter(|candidate| edition.offers_touch_scheme(*candidate))
                .collect();
            assert_eq!(offered, [scheme], "{id}");
        }
    }

    fn artifact_names(set: &ResourceSet) -> BTreeSet<&str> {
        set.artifacts
            .iter()
            .map(|artifact| artifact.name.as_str())
            .collect()
    }

    /// full 的资源锁逐字节就是原文件，代次与引入版本之前相同：用户词库目录 `user/dictionaries/<代次>` 不变，升级不会重新准备。期望值是按当前这份锁（msime-dictionary 的 `dict-v2.0.7`）算出的代次；换词库版本时它理应变化，届时连同锁文件一起更新。
    #[test]
    fn full_keeps_the_desktop_lock_and_its_generation() {
        let full = Edition::full();
        assert_eq!(
            full.resource_lock(),
            Some(include_str!(
                "../../../resources/desktop-dictionary.lock.json"
            ))
        );
        let set = full.resource_set().unwrap();
        assert_eq!(set.artifacts.len(), 12);
        assert_eq!(
            set.generation().unwrap(),
            "b7c435956c0ad609f0350b4d26c8c18943df08c2b91621d8194904f5b9be8238"
        );
    }

    /// 每个版本都有资源锁，锁里的文件恰好是它的组件的并集，每个条目与原文件的同名条目逐字段相同。
    #[test]
    fn every_edition_lock_is_the_union_of_its_components() {
        let table: serde_json::Value = serde_json::from_str(EDITIONS_JSON).unwrap();
        let components = table["resource_components"].as_object().unwrap();
        let full = Edition::full().resource_set().unwrap();
        for edition in Edition::all() {
            let set = edition.resource_set().unwrap();
            let expected: BTreeSet<&str> = edition
                .resources
                .components
                .iter()
                .flat_map(|component| components[component].as_array().unwrap())
                .map(|name| name.as_str().unwrap())
                .collect();
            assert_eq!(artifact_names(&set), expected, "{}", edition.id);
            assert_eq!(set.source_commit, full.source_commit, "{}", edition.id);
            for artifact in &set.artifacts {
                let original = full
                    .artifacts
                    .iter()
                    .find(|candidate| candidate.name == artifact.name)
                    .unwrap();
                assert_eq!(artifact.sha256, original.sha256, "{}", artifact.name);
                assert_eq!(artifact.size, original.size, "{}", artifact.name);
                assert_eq!(artifact.url, original.url, "{}", artifact.name);
            }
        }
    }

    /// 五笔版不带临时日文和整句重排模型，带五笔码表 msime-wubi.db、五笔混拼要用的 msime-pinyin.db 和整句词格要用的 n-gram 表。
    #[test]
    fn the_wubi_lock_leaves_out_japanese_and_the_sentence_model() {
        let set = Edition::by_id("wubi").unwrap().resource_set().unwrap();
        assert_eq!(
            artifact_names(&set),
            BTreeSet::from([
                "msime-bigram.bin",
                "msime-dictionary-manifest.json",
                "msime-english.db",
                "msime-pinyin.db",
                "msime-others.db",
                "msime-scowl_Copyright.txt",
                "msime-trigram.bin",
                "msime-wubi.db",
            ])
        );
        assert_ne!(
            set.generation().unwrap(),
            Edition::full()
                .resource_set()
                .unwrap()
                .generation()
                .unwrap()
        );
    }

    /// 日文、越南文和藏文版只带核心资源（英文词库及其 SCOWL 许可声明、符号表和清单），日文版另带日文词典和两份 Mozc 许可文本；三者都不带 msime-pinyin.db、msime-wubi.db、n-gram 表和整句模型。
    #[test]
    fn the_language_edition_locks_carry_no_chinese_dictionary() {
        let core = BTreeSet::from([
            "msime-dictionary-manifest.json",
            "msime-english.db",
            "msime-others.db",
            "msime-scowl_Copyright.txt",
        ]);
        let mut japanese = core.clone();
        japanese.extend([
            "msime-japanese.dat",
            "msime-mozc_dictionary_oss_README.txt",
            "msime-mozc_LICENSE.txt",
        ]);
        let full_generation = Edition::full()
            .resource_set()
            .unwrap()
            .generation()
            .unwrap();
        let mut generations = BTreeSet::new();
        for (id, expected) in [
            ("japanese", japanese),
            ("vietnamese", core.clone()),
            ("tibetan", core.clone()),
        ] {
            let set = Edition::by_id(id).unwrap().resource_set().unwrap();
            assert_eq!(artifact_names(&set), expected, "{id}");
            let generation = set.generation().unwrap();
            assert_ne!(generation, full_generation, "{id}");
            generations.insert(generation);
        }
        // 越南文和藏文的文件清单相同，代次也相同；它们的状态目录各自独立，所以不会共用一个代次目录。
        assert_eq!(generations.len(), 2);
    }

    #[test]
    fn a_misspelled_preference_default_is_rejected() {
        let error = serde_json::from_str::<PreferenceDefaults>(r#"{"wubi_mixed_pinyn": true}"#)
            .unwrap_err();
        assert!(error.to_string().contains("wubi_mixed_pinyn"), "{error}");
    }

    fn account_settings(scheme: &str) -> BTreeMap<String, AccountPreferenceValue> {
        BTreeMap::from([
            (
                "input.schema".to_owned(),
                AccountPreferenceValue::String(scheme.to_owned()),
            ),
            (
                "platform.ios.nine_key".to_owned(),
                AccountPreferenceValue::Boolean(true),
            ),
            (
                "platform.android.keyboard_layout".to_owned(),
                AccountPreferenceValue::String("nine_key".to_owned()),
            ),
            (
                "input.shuangpin_schema".to_owned(),
                AccountPreferenceValue::String("ziranma".to_owned()),
            ),
            (
                "input.wubi_schema".to_owned(),
                AccountPreferenceValue::String("wubi98".to_owned()),
            ),
            (
                "input.learning".to_owned(),
                AccountPreferenceValue::Boolean(false),
            ),
        ])
    }

    fn keys(settings: &BTreeMap<String, AccountPreferenceValue>) -> Vec<&str> {
        settings.keys().map(String::as_str).collect()
    }

    #[test]
    fn full_syncs_every_account_setting_unchanged() {
        let full = Some(Edition::full());
        for scheme in [
            "quanpin",
            "shuangpin",
            "wubi",
            "japanese",
            "korean",
            "cantonese",
            "klingon",
        ] {
            let mut uploaded = account_settings(scheme);
            filter_uploaded_account_settings(full, &mut uploaded);
            assert_eq!(uploaded, account_settings(scheme), "{scheme}");
            let mut downloaded = account_settings(scheme);
            filter_downloaded_account_settings(full, &mut downloaded);
            assert_eq!(downloaded, account_settings(scheme), "{scheme}");
        }
    }

    #[test]
    fn a_single_scheme_edition_neither_uploads_nor_applies_the_scheme() {
        let wubi = Edition::by_id("wubi");
        for scheme in ["quanpin", "wubi", "klingon"] {
            let mut uploaded = account_settings(scheme);
            filter_uploaded_account_settings(wubi, &mut uploaded);
            // 五笔版本照常上传（本版本就是五笔），双拼方案只是本机缺省值，不上传。
            assert_eq!(
                keys(&uploaded),
                ["input.learning", "input.wubi_schema"],
                "{scheme}"
            );
            let mut downloaded = account_settings(scheme);
            filter_downloaded_account_settings(wubi, &mut downloaded);
            assert_eq!(
                keys(&downloaded),
                [
                    "input.learning",
                    "input.shuangpin_schema",
                    "input.wubi_schema"
                ],
                "{scheme}"
            );
        }
    }

    #[test]
    fn a_multi_scheme_edition_syncs_only_its_own_schemes() {
        let pinyin = Edition::by_id("pinyin");
        for scheme in ["quanpin", "shuangpin"] {
            let mut uploaded = account_settings(scheme);
            filter_uploaded_account_settings(pinyin, &mut uploaded);
            assert_eq!(
                keys(&uploaded),
                [
                    "input.learning",
                    "input.schema",
                    "input.shuangpin_schema",
                    "platform.android.keyboard_layout",
                    "platform.ios.nine_key"
                ],
                "{scheme}"
            );
            let mut downloaded = account_settings(scheme);
            filter_downloaded_account_settings(pinyin, &mut downloaded);
            assert_eq!(downloaded, account_settings(scheme), "{scheme}");
        }
        // 账号里是本版本没有的方案：当作没有这一项，本机方案不变，其余设置照常应用。
        for scheme in ["wubi", "japanese", "korean", "cantonese"] {
            let mut downloaded = account_settings(scheme);
            filter_downloaded_account_settings(pinyin, &mut downloaded);
            assert_eq!(
                keys(&downloaded),
                [
                    "input.learning",
                    "input.shuangpin_schema",
                    "input.wubi_schema"
                ],
                "{scheme}"
            );
            let mut uploaded = account_settings(scheme);
            filter_uploaded_account_settings(pinyin, &mut uploaded);
            assert_eq!(
                keys(&uploaded),
                ["input.learning", "input.shuangpin_schema"],
                "{scheme}"
            );
        }
        // 不认识的取值留给平台代码，按原来的规则保留本机方案。
        let mut downloaded = account_settings("klingon");
        filter_downloaded_account_settings(pinyin, &mut downloaded);
        assert!(downloaded.contains_key("input.schema"));
    }

    #[test]
    fn full_keeps_its_macos_identifiers_and_every_edition_has_its_own() {
        let full = Edition::full().macos().unwrap();
        assert_eq!(
            full.input_method_bundle_id,
            "app.msime.inputmethod.MetasequoiaIME"
        );
        assert_eq!(full.input_method_bundle_name(), "水杉输入法.app");
        assert_eq!(full.settings_bundle_id, "app.msime.macos");
        assert_eq!(full.keychain_service, "com.metasequoia.msime.account");
        assert_eq!(full.cask, "msime");
        assert_eq!(full.dmg_prefix, "msime-macos");
        let wubi = Edition::by_id("wubi").unwrap().macos().unwrap();
        assert_eq!(wubi.input_method_bundle_name(), "水杉五笔.app");
        // 还没有 macOS 段的版本（值为 null）不参与比较，由引入该平台的阶段补齐。
        let sections: Vec<_> = Edition::all().iter().filter_map(Edition::macos).collect();
        assert!(sections.len() >= 3);
        let bundles: BTreeSet<_> = sections
            .iter()
            .map(|section| section.input_method_bundle_id.to_lowercase())
            .collect();
        assert_eq!(bundles.len(), sections.len());
        let states: BTreeSet<_> = sections
            .iter()
            .map(|section| section.settings_bundle_id.to_lowercase())
            .collect();
        assert_eq!(states.len(), sections.len());
    }

    #[test]
    fn full_keeps_its_windows_identifiers_and_the_others_add_a_suffix() {
        let full = Edition::full().windows().unwrap();
        assert_eq!(full.clsid, "{E3062E9A-D834-4637-8958-ED8CFA427D01}");
        assert_eq!(full.registry_key, r"Software\Metasequoia\MetasequoiaIME");
        assert_eq!(full.state_directory, "MSIME-Client");
        assert_eq!(
            full.data_dir_environment_variable,
            "METASEQUOIA_IME_DATA_DIR"
        );
        assert_eq!(full.host_dll, "msime_host_api.dll");
        assert_eq!(full.tsf_guids.len(), 14);
        assert_eq!(
            full.pipe_name("FanyImeAuxNamedPipe"),
            r"\\.\pipe\FanyImeAuxNamedPipe"
        );
        assert_eq!(
            full.named(r"Local\MSIME.Client.ClipboardHistoryChanged"),
            r"Local\MSIME.Client.ClipboardHistoryChanged"
        );
        let wubi = Edition::by_id("wubi").unwrap().windows().unwrap();
        assert_eq!(
            wubi.pipe_name("FanyImeAuxNamedPipe"),
            r"\\.\pipe\FanyImeAuxNamedPipe.wubi"
        );
        assert_eq!(wubi.state_directory, "MSIME-Client-wubi");
        // 还没有 Windows 段的版本（值为 null）不参与比较，由引入该平台的阶段补齐。
        let sections: Vec<_> = Edition::all().iter().filter_map(Edition::windows).collect();
        assert!(sections.len() >= 3);
        let pipes: BTreeSet<_> = sections
            .iter()
            .map(|section| section.pipe_name("FanyImeNamedPipe"))
            .collect();
        assert_eq!(pipes.len(), sections.len());
        let clsids: BTreeSet<_> = sections
            .iter()
            .map(|section| section.clsid.to_uppercase())
            .collect();
        assert_eq!(clsids.len(), sections.len());
    }

    #[test]
    fn full_keeps_its_linux_identifiers_and_the_others_derive_theirs_from_the_id() {
        let full = Edition::full().linux().unwrap();
        assert_eq!(full.package, "msime-linux");
        assert_eq!(full.install_prefix, "/usr");
        assert_eq!(full.client_directory, "msime-client");
        assert_eq!(full.ibus_engine, "msime-linux");
        assert_eq!(full.fcitx5_addon, "msime");
        assert_eq!(full.tauri_identifier, "app.msime.linux");
        assert_eq!(full.user_unit("voice.socket"), "msime-linux-voice.socket");
        assert_eq!(full.setup_program(), "msime-linux-setup");
        assert_eq!(full.settings_program(), "msime-linux-settings");
        let wubi = Edition::by_id("wubi").unwrap().linux().unwrap();
        assert_eq!(wubi.package, "msime-linux-wubi");
        assert_eq!(wubi.install_prefix, "/opt/msime-linux-wubi");
        assert_eq!(wubi.client_directory, "msime-client-wubi");
        assert_eq!(wubi.ibus_engine, "msime-linux-wubi");
        assert_eq!(wubi.fcitx5_addon, "msime-wubi");
        assert_eq!(
            wubi.user_unit("voice.socket"),
            "msime-linux-wubi-voice.socket"
        );
        assert_eq!(wubi.setup_program(), "msime-linux-wubi-setup");
        // 还没有 Linux 段的版本（值为 null）不参与比较，由引入该平台的阶段补齐。
        let sections: Vec<_> = Edition::all().iter().filter_map(Edition::linux).collect();
        assert!(sections.len() >= 3);
        let directories: std::collections::HashSet<_> = sections
            .iter()
            .map(|section| section.client_directory.clone())
            .collect();
        assert_eq!(directories.len(), sections.len());
    }

    #[test]
    fn a_test_process_without_a_marker_beside_it_runs_as_linux_full() {
        assert!(Edition::of_linux_package().unwrap().is_full());
        assert_eq!(
            Edition::linux_package_identity().unwrap().client_directory,
            "msime-client"
        );
    }

    #[test]
    fn a_test_process_without_a_marker_beside_it_runs_as_windows_full() {
        assert!(Edition::of_windows_package().unwrap().is_full());
        assert_eq!(
            Edition::windows_package_identity().unwrap().state_directory,
            "MSIME-Client"
        );
    }

    #[test]
    fn a_test_process_is_not_inside_a_macos_bundle_and_runs_as_full() {
        assert!(Edition::of_macos_bundle().unwrap().is_full());
    }

    #[test]
    fn a_package_without_a_marker_is_full_and_a_bad_marker_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join(Edition::PACKAGE_MARKER_FILE);
        assert!(Edition::declared_by_package(&marker).unwrap().is_full());
        std::fs::write(&marker, br#"{"edition":"wubi"}"#).unwrap();
        assert_eq!(Edition::declared_by_package(&marker).unwrap().id, "wubi");
        for bad in [
            &br#"{"edition":"klingon"}"#[..],
            br#"{"edition":3}"#,
            br#"{"edition":"wubi","extra":1}"#,
            b"wubi",
        ] {
            std::fs::write(&marker, bad).unwrap();
            assert!(Edition::declared_by_package(&marker).is_err());
        }
    }

    #[test]
    fn an_unknown_edition_does_not_touch_the_scheme() {
        let mut uploaded = account_settings("quanpin");
        filter_uploaded_account_settings(None, &mut uploaded);
        assert_eq!(keys(&uploaded), ["input.learning"]);
        let mut downloaded = account_settings("quanpin");
        filter_downloaded_account_settings(None, &mut downloaded);
        assert_eq!(
            keys(&downloaded),
            [
                "input.learning",
                "input.shuangpin_schema",
                "input.wubi_schema"
            ]
        );
    }
}
