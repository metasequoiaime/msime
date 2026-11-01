import type { InputScheme } from "./input-scheme";

export type HostPlatform = "windows" | "macos" | "linux" | "android" | "ios" | "harmony";
/** 对应 `client-core::host_surface::HostCapabilities`。 */
export interface HostCapabilities {
  platform: HostPlatform;
  /** 签名 HAP 带有数据组 id 时，HarmonyOS 输入法扩展与设置 Ability 才能共享文件。 */
  harmony_shared_sandbox?: boolean;
  /** 设置是否使用手机导航与触屏界面。 */
  mobile_settings: boolean;
  /** 宿主实体键盘支持可选的韩语罗马字布局。 */
  korean_romaja?: boolean;
  restart_input_method: boolean;
  panel_windows: boolean;
  ime_mode_scope: boolean;
  /** 宿主按 `app_input_mode_rules` 给指定应用定起始中英文模式（macOS、Windows 和 Linux）。 */
  app_input_mode_rules?: boolean;
  /** 宿主对 `vim_mode_apps` 名单里的应用把无修饰 Esc 搭车切英文（目前只有 macOS）。 */
  vim_mode_apps?: boolean;
  typing_statistics: boolean;
  /** 宿主已接通共享的背单词入口。 */
  vocabulary_review: boolean;
  fuzzy_pinyin: boolean;
  system_fonts: boolean;
  window_chrome: boolean;
  floating_toolbar: boolean;
  floating_toolbar_appearance: boolean;
  floating_toolbar_components: boolean;
  /** 工具栏带手写面板按钮，macOS、Windows 和 Linux 的工具栏提供（手写还要随版本收窄）。 */
  floating_toolbar_handwriting: boolean;
  /** 工具栏带语音输入按钮，macOS、Windows 和 Linux 的工具栏提供。 */
  floating_toolbar_voice: boolean;
  /** 工具栏带切换输入方案的按钮（macOS、Windows 和 Linux）。 */
  floating_toolbar_input_scheme: boolean;
  /** 宿主按 `floating_toolbar.auto_hide` / `auto_hide_delay` / `shadow` 控制悬浮条；目前只有 macOS 消费，其余宿主不列这三行。 */
  floating_toolbar_idle_behavior?: boolean;
  mode_switch_shortcuts: boolean;
  panel_shortcuts: boolean;
  number_row_selection: boolean;
  /** 宿主没有桌面面板窗口也响应外接键盘的录音快捷键（HarmonyOS 手机），设置页因而仍提供这些开关。 */
  voice_hotkeys?: boolean;
  voice_capture_devices: boolean;
  candidate_font_controls: boolean;
  candidate_preedit_font: boolean;
  candidate_page_number: boolean;
  /** 宿主按 `show_app_logo` 显示或隐藏候选窗和悬浮工具栏左端的水杉 logo（macOS、Windows 和 Linux 的 Fcitx5）。 */
  app_logo?: boolean;
  /** 宿主按 `traditional_chinese_variant` 选 OpenCC 的 s2t、s2tw 或 s2twp 做繁体转换（目前只有 Linux）；没有时设置页只给简体和繁体。 */
  traditional_chinese_variants?: boolean;
  /** 宿主按 `expand_candidates` 让横排候选窗按 Tab、↓ 展开成多行候选网格（macOS，以及 X11 上用原生画布画候选窗的 Linux Fcitx5）。 */
  expand_candidates?: boolean;
  /** 宿主画的各个界面一律跟随颜色模式，不读 `settings_theme` 等各界面的单独明暗，只有候选窗口仍读 `candidate_theme`（macOS、Windows、Linux 和 HarmonyOS；Android 和 iOS 不是）。 */
  surface_themes_follow?: boolean;
  candidate_row_colors: boolean;
  candidate_selection_appearance: boolean;
  /** 宿主按用户选的边框色给候选窗描边。Linux 由 Fcitx5 的 classicui 主题描边，没有 hover 状态；桌面宿主（macOS、Windows、Linux 和 HarmonyOS）的边框色固定跟随主题和皮肤，不读用户选的值，只有 Android 和 iOS 照旧读。 */
  candidate_border_color: boolean;
  /** 宿主按用户选的编号色画候选编号。桌面宿主（macOS、Windows、Linux 和 HarmonyOS）的编号色固定跟随主题（选了文字颜色时由它推出），不读用户选的值，只有 Android 和 iOS 照旧读；缺省按读取处理。 */
  candidate_number_color?: boolean;
  /** 宿主绘制独立候选窗，并按 `candidate_scale_percent` 缩放字体与几何尺寸。 */
  candidate_window_scale: boolean;
  /** 宿主按 `candidate_opacity_percent` 降低候选卡片底色、边框及皮肤背景的不透明度，文字保持不透明。 */
  candidate_window_opacity: boolean;
  /** 宿主按 `candidate_corner_radius` 设置候选卡片圆角，优先于皮肤包里的半径。 */
  candidate_corner_radius: boolean;
  /** 宿主按 `candidate_padding` 设置卡片边缘与候选之间的内边距，优先于皮肤包；目前是 macOS 和 Linux 的 Fcitx5。 */
  candidate_padding: boolean;
  /** 窗口分轴内边距与候选项的内边距、圆角和间距；旧宿主不显示这些控件。 */
  candidate_item_geometry?: boolean;
  candidate_follow_cursor: boolean;
  input_mode_hud: boolean;
  /** 宿主在系统菜单栏放一个常驻状态项显示当前输入模式。只有 macOS 提供：Windows 有托盘语言栏，Linux 有输入法面板，触屏键盘的键面已显示模式。 */
  input_mode_status_item: boolean;
  candidate_english_font: boolean;
  english_suggestions: boolean;
  english_online_candidates?: boolean;
  helpcode_shift_entry: boolean;
  /** 宿主已接通辅助码前导键。 */
  helpcode_trigger_key?: boolean;
  skin_directory_import: boolean;
  /** 设置页提供「制作皮肤」标签：宿主经 client-services 的 `skin::candidate_editor` 把编辑器做的皮肤装进本机皮肤目录（`SettingsClient.candidateSkinEditor`）。桌面宿主为真；HarmonyOS 按设备形态只在 2in1 上打开。缺省按不提供处理。 */
  candidate_skin_editor?: boolean;
  /** 宿主只支持一种每页候选数时固定为此值，不提供选择。 */
  fixed_candidate_page_size?: number;
  /** 宿主能排的最大每页候选数：macOS、Linux、Android 为 10（第十个用 0 键选），其余为 9。 */
  max_candidate_page_size?: number;
  /** 宿主固定使用的 AI 联想候选数量（macOS、Windows、Linux、鸿蒙为 3）；有它时设置页不显示「候选数量」，宿主也不读文档里存的值。 */
  fixed_ai_candidate_limit?: number;
  /** 宿主固定使用的打字效果强度（macOS、Windows、鸿蒙为 50）；有它时设置页不显示「效果强度」，宿主也不读文档里存的值。 */
  fixed_effect_intensity?: number;
  /** 宿主只支持一种候选排列时固定为此值，不提供选择。 */
  fixed_candidate_layout?: "horizontal" | "vertical";
  /** 触屏键盘按 `touch_toolbar` 选择工具栏按钮。 */
  touch_toolbar_components: boolean;
  /** 触屏键盘按 `harmony_toolbar` 选工具栏的工具和显示方式（只有 HarmonyOS 手机）。缺省按不提供处理，其他宿主的页面不变。 */
  touch_toolbar_tools?: boolean;
  /** 触屏键盘的按键音按 `plugins.key_sound.pack` 选音效包，只播包里的按键样本，不播旋律、上屏音和成就音；屏幕键盘页「按键反馈」组在「按键音」下给出「音效」选择。目前只有 HarmonyOS 手机和平板的设置桥按设备形态报告（`SettingsFormFactorCapabilities`），2in1 的音效包仍在插件页里选。缺省按不提供处理。 */
  touch_key_sound_packs?: boolean;
  /** 触屏设置里的「插件」页：宿主的触屏键盘读插件目录，设置页在手机导航里给出插件页，只列在触屏键盘上生效的数据插件（按键音效包、指令表、短语表、辅助码表、单词本和符号集，按各自的能力），按键音效包按触屏规则选用（开关在「按键反馈」，只播按键音），导入只收 .zip，社区插件也只列这几类。目前只有 HarmonyOS 手机和平板的设置桥按设备形态报告（`SettingsFormFactorCapabilities`）；2in1 走桌面形态的插件页。缺省按不提供处理，其他触屏宿主不显示插件页。 */
  touch_plugin_packs?: boolean;
  /** 触屏键盘在大屏单手模式空出来的一侧按 `touch_one_handed_side_panel` 画数字键盘，屏幕键盘页「布局」组给出「大屏单手时侧边显示数字键盘」。目前只有 HarmonyOS 的设置桥按设备形态报告（`SettingsFormFactorCapabilities`：平板和可折叠手机为真，直板手机和 2in1 为假）；Android 和 iOS 在原生设置页里给出这一项。缺省按不提供处理。 */
  touch_one_handed_side_panel?: boolean;
  /** 触屏键盘在大屏横屏时按 `touch_keyboard_split`、`touch_keyboard_split_portrait`、`touch_keyboard_split_threshold` 和 `touch_keyboard_split_gap` 画分离式键盘，屏幕键盘页「布局」组给出「分离式键盘」「竖屏也分离」「分离启用宽度」和「分离中间空隙」。目前只有 HarmonyOS 的设置桥按设备形态报告（`SettingsFormFactorCapabilities`：平板和可折叠手机为真，直板手机和 2in1 为假）；Android 和 iOS 在原生设置页里给出这几项。缺省按不提供处理。 */
  touch_keyboard_split?: boolean;
  /** 宿主按 `touch_keyboard_height_adjustment` 改变屏幕键盘的高度（Windows 与触屏宿主）；macOS 和 Linux 的键盘窗口高度固定，不给「键盘高度」滑块。缺省按提供处理。 */
  touch_keyboard_height?: boolean;
  /** 宿主的屏幕键盘按 `touch_key_spacing_tenths` / `touch_row_spacing_tenths` 画间距；macOS 为 false，键盘面板固定用默认间距，设置页不给间距滑块、拖动调间距和「恢复默认」。缺省按提供处理。 */
  touch_keyboard_spacing?: boolean;
  shuangpin_preedit: boolean;
  /** 宿主在双拼组字时按 `shuangpin_keymap_hint` 在候选窗旁画键位图（macOS、Windows 和 Linux 的 X11 客户端）。 */
  shuangpin_keymap_hint?: boolean;
  /** 宿主响应 Ctrl+Shift+Alt 维护快捷键。 */
  maintenance_shortcuts: boolean;
  /** 宿主保留 Option/Alt+Shift+H 用于切换字符宽度。 */
  fullwidth_chord: boolean;
  /** 宿主按 `keybindings.toggle_*` 认三个切换热键，快捷键页给出录键控件；缺省按不提供处理。 */
  toggle_hotkey_chords?: boolean;
  /** 宿主运行配置的转写服务，因此相关设置会生效。 */
  voice_provider_settings: boolean;
  /** 宿主绘制临时识别文字，因此流式预编辑开关会生效。 */
  voice_stream_preedit: boolean;
  /** 宿主读 `voice_input.hotkey_hold_space_lock`，所以给出空格锁定开关。macOS、Windows、Linux、HarmonyOS 固定为开，报 false；只有 Android、iOS 报 true；缺省按 true。 */
  voice_space_lock_switch?: boolean;
  /** 宿主分别读开始、结束提示音，所以在总开关之外给出这两个开关。macOS、Windows、Linux、HarmonyOS 只认总开关，报 false；只有 Android、iOS 报 true；缺省按 true。 */
  voice_cue_switches?: boolean;
  /** 宿主在点按开始的本地识别里按 `voice_input.pause_millis` 和 `voice_input.stop_sensitivity` 结束录音，语音页给出「本地停顿时间」和「判停灵敏度」。目前只有 HarmonyOS 的设置桥在手机和平板上报告（`SettingsFormFactorCapabilities`，2in1 不按停顿结束）；Android 和 iOS 在原生设置页里给出这两项。缺省按不提供处理。 */
  voice_pause_settings?: boolean;
  /** 宿主有不止一种录音后端，所以给出「录音后端」和手填设备名。只有 Linux 报 true；macOS、Windows、HarmonyOS 各只有一种录音方式，报 false，设置页只给「麦克风」下拉；缺省按 true。 */
  voice_capture_backends?: boolean;
  /** 宿主向运行时报告字符宽度，因此全角输入能够生效。 */
  character_width: boolean;
  ai_provider_credentials: boolean;
  /** 宿主提供 AI 生成类功能（AI 对话、AI 润色与高情商回复、AI 联想候选、AI 翻译、语音润色和 AI 皮肤）。为 false 时设置页不给 AI 辅助页、AI 对话和其余这些入口；只有 HarmonyOS 在拿到 AI 生成类资质之前报 false。缺省按提供处理。 */
  ai_generation?: boolean;
  /** 宿主提供社区投稿类功能：发布皮肤、词包、短语、回复模板和插件，修改已发布作品的分类，发布新版本，给他人作品评分和举报。为 false 时社区页只能浏览、搜索、看详情、获取、安装、收藏、导入和下架自己的作品；只有 HarmonyOS 在实名认证到位之前报 false。缺省按提供处理。 */
  community_contribution?: boolean;
  voice_commit_mode: boolean;
  /** 宿主运行的操作系统版本，供反馈页附带。 */
  os_version?: string;
  /** 宿主编译时的 CPU 架构（Rust 的 `std::env::consts::ARCH`，如 `x86_64`、`aarch64`）；更新检查据此选择本机的 Linux 安装包。 */
  arch?: string;
  /** 内核版本（Linux 的 `/proc/sys/kernel/osrelease`），「关于」页系统信息用；宿主读不到时缺省。 */
  kernel_version?: string;
  /** 桌面会话（Linux 的 `XDG_CURRENT_DESKTOP` 与 `XDG_SESSION_TYPE`，如 `ubuntu:GNOME (wayland)`）。 */
  desktop_session?: string;
  /** 水杉实际挂在哪个输入法框架上（Linux 的 `IBus` / `Fcitx5`），取自运行中的宿主写的状态。 */
  input_method_framework?: string;
  /** 设备型号（Linux 的 DMI 厂商与产品名）。 */
  device_model?: string;
  /** 运行中的 Linux 宿主报告的、画候选列表的桌面面板为什么不理会候选字体、颜色和皮肤；面板全都照办时缺省。`ibus_panel` 是部分不生效：IBus 面板接收字体和颜色，不接收圆角、内边距、不透明度、水杉 logo 和皮肤的装饰图，也不描边（Linux 本来就不给边框色取色器）。 */
  candidate_panel_limit?: "gnome_shell" | "fcitx_theme" | "kimpanel" | "ibus_panel";
  /** 宿主播放 `plugins` 中的音效包：按键音、旋律、上屏音和成就提示音。 */
  key_sound: boolean;
  /** 宿主处理 V、/、@ 模式：把 / 和 @ 交给运行时；模式使用数字时保留数字输入，并加载命令表与 @ 名称表。 */
  plugin_triggers: boolean;
  /** 宿主作为当前输入法时，流式播放选中的音乐包。 */
  music: boolean;
  /** 宿主绘制打字特效及 `msime_client_typing_effect` 返回的连击计数。 */
  typing_effects: boolean;
  /** 宿主向会话报告大写锁定状态，「大写锁定时使用英文标点」在这里有效（macOS 和 Linux）。 */
  caps_lock_punctuation?: boolean;
  /** 宿主能分辨 Caps Lock 的轻点和长按并撤回大写锁定，「单按 Caps Lock 切换中英文」在这里有效（三个桌面宿主）。 */
  caps_lock_language_switch?: boolean;
  /** 快捷模式只从实体键盘的 Shift+字母进入，「Shift+字母进入快捷模式」总开关在这里有意义（三个桌面宿主）。 */
  shift_letter_entry_switch?: boolean;
  /** 宿主把快捷短语编码追加到普通候选（macOS 和 Linux）。 */
  quick_phrase_candidates?: boolean;
  /** 宿主提供「移动光标后编辑光标后的部分」（`caret_edit_suffix`，三个桌面宿主）。缺省（早于这一项的宿主）按不提供处理。 */
  caret_edit_suffix?: boolean;
  /** 宿主显示上屏后联想（`next_word_prediction`）：没有预编辑时显示候选，只有点选才上屏。各宿主接好之后才声明。缺省（早于这一项的宿主）按不提供处理。 */
  next_word_prediction?: boolean;
  /** 宿主处理 Ctrl+Shift+J 在中文和日语之间切换（Windows 和 Linux）。本版本还要同时提供中文方案和日语，设置页才显示它的开关。 */
  japanese_toggle_shortcut?: boolean;
  /** 宿主把「切换中英文」那一组快捷键接进了中日英循环（目前只有 macOS）。本版本还要同时提供中文方案和日语，设置页才显示它的开关。 */
  trilingual_cycle?: boolean;
  /** 宿主读 `navigation.tab` 和 `navigation.page_up_down`，设置页给出这两组翻页键的开关。macOS、Windows 和 HarmonyOS 为假：关掉只会让键在组字时被吃掉，两组固定开启。缺省（早于这一项的宿主）按读处理。 */
  tab_page_paging_switches?: boolean;
  /** 宿主读 `mixed_input.minimum_prefix`，设置页给出「触发字符数」。macOS、Windows 和 Linux 为假：固定为 5。缺省按读处理。 */
  mixed_input_minimum_prefix?: boolean;
  /** 背单词书目列出单词本插件（`pack-<插件 id>` 词书）。 */
  wordbook_packs: boolean;
  /** 符号面板显示已安装的符号集插件。 */
  symbol_set_packs: boolean;
  /** 宿主能用系统语音读出背单词卡片上的英文词头，卡片上画喇叭。缺省（早于这一项的宿主）按不能读处理。 */
  read_aloud?: boolean;
  /** 插件页显示「脚本插件（实验）」分组和授权页（macOS 和 Linux 的宿主）。 */
  script_plugins?: boolean;
  /** 宿主提供的输入方案；其他方案显示为禁用。 */
  input_schemes: InputScheme[];
  /** 运行中的版本，不是 full 时才有。缺省就是 full：所有方案都属于本版本，`input_schemes` 之外的方案只是这个宿主暂不支持，显示为禁用；有它时，`edition.input_schemes` 之外的方案在本版本里不存在，设置页直接不列出。 */
  edition?: EditionInfo;
}

/** 对应 `client-core::host_surface::EditionInfo`。 */
export interface EditionInfo {
  id: string;
  /** 版本的中文产品名，例如「水杉五笔」。缺省时按 full 的「水杉输入法」。 */
  display_name?: string;
  /** 本版本提供的方案，顺序与全部方案的顺序一致。 */
  input_schemes: InputScheme[];
  /** 本版本的默认方案，偏好里的方案不可用时 host-api 回退到它。 */
  default_scheme: InputScheme;
  /** 本版本是否带临时日文。 */
  temporary_japanese: boolean;
  /** 本版本是否带键盘神经联想用的模型。不带时触屏宿主不列出神经联想开关；桌面的神经联想用另一份模型，不归这一项管。 */
  neural_keyboard: boolean;
  /** 本版本是否带非英文目标语言的离线候选释义。它们按中文候选查，所以只有提供中文方案的版本带。 */
  offline_glosses: boolean;
  /** 本版本是否提供手写。手写模型只认汉字，不提供中文方案的版本（日文、越南文和藏文版）没有手写：设置页不列出手写页，宿主也不打开手写面板、不下载模型。 */
  handwriting: boolean;
  /** 本版本里五笔混拼的默认值。 */
  wubi_mixed_pinyin_default: boolean;
}
