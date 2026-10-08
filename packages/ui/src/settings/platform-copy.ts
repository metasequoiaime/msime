export interface PlatformCopyContext {
  android: boolean;
  linux: boolean;
  macos: boolean;
  harmony: boolean;
  ios: boolean;
  mobile: boolean;
}

export interface PlatformCopy {
  helpIntro: string;
  quickStart: string;
  networkDescription: string;
  aboutDescription: string;
}

/** Platform-specific prose shared by the help and about settings pages. */
export function platformCopy({
  android,
  linux,
  macos,
  harmony,
  ios,
  mobile,
}: PlatformCopyContext): PlatformCopy {
  const helpIntro = android
    ? "水杉输入法是一款 Android 平台的中文输入法，通过系统输入法服务接入应用。"
    : linux
      ? "水杉输入法是一款 Linux 桌面环境下的中文输入法，通过 Fcitx5 或 IBus 接入 GTK、Qt 等应用。"
      : macos
        ? "水杉输入法是一款 macOS 平台的中文输入法，通过系统输入法组件接入应用。"
        : harmony
          ? "水杉输入法是一款 HarmonyOS 平台的中文输入法，通过系统输入法服务接入应用。"
          : ios
            ? "水杉输入法是一款 iOS 平台的中文输入法，通过键盘扩展接入应用。"
            : "水杉输入法是一款 Windows 平台的中文输入法。目前支持 Windows 11/Windows 10 平台。";
  const quickStart = android
    ? "在系统设置的“语言和输入法”或“屏幕键盘”中启用并选择水杉输入法，也可以从首次启动页打开这些入口。默认是全拼输入法。"
    : linux
      ? "首次配置（首次配置页或 msime-linux-setup）完成后会把水杉输入法自动加入正在运行的 Fcitx5 或 IBus 的输入法列表，之后用输入法切换快捷键切换即可。未能自动加入时手动添加：使用 Fcitx5 时，用 fcitx5-configtool 把「水杉输入法」（英文界面显示为「MSIME」）加入当前输入法组；使用 IBus 时，执行 ibus restart 后在系统设置的输入源中添加「Metasequoia 水杉输入法」。默认是全拼输入法。"
      : macos
        ? "设置应用每次启动时会自动安装或更新随附的水杉输入法，并在系统设置的键盘输入法中启用它；首次安装后如提示需要重新登录，注销并重新登录一次即可。之后使用系统配置的输入法切换快捷键。默认是全拼输入法。"
        : harmony
          ? mobile
            ? "在系统设置中启用并选择水杉输入法，再从输入法键盘使用语音和触屏输入。默认是全拼输入法。"
            : "在系统设置中启用并选择水杉输入法，再使用实体键盘、候选窗口和悬浮工具栏输入。默认是全拼输入法。"
          : ios
            ? "在系统设置中启用水杉键盘，再从应用的输入源按钮切换使用。默认是全拼输入法。"
            : "安装输入法后，可以使用 Win + Space 快捷键切换到水杉输入法。默认是全拼输入法。";
  const networkDescription = android
    ? "语音输入会调用设备上的系统语音识别服务，在键盘里聆听并直接插入识别结果；AI 功能按需配置。日常拼音输入无需联网。"
    : linux
      ? "日常拼音输入无需联网。云候选默认关闭（首次配置时可以启用，之后也可在设置里改），开启时会把正在输入的拼写发给 Google input-tools 换回一条候选；候选词翻译默认不联网，在翻译服务里选择自己的服务，或选择水杉账号把当前页的中文候选词发送到 api.msime.app 之后才会发请求；语音识别和 AI 功能只在启用并配置好对应服务后联网。这些请求由用户级的 msime-linux-online-provider 和 msime-linux-voice-provider 服务发出，输入法本身不联网。Linux 安装后的用户初始化会自动注册本机匿名水杉账号，网络失败时稍后重试。在 AI、腾讯翻译和语音页面填写的凭据只写入用户配置目录（通常是 ~/.config/msime-client）下仅本人可读的 ai-provider.json、tencent-provider.json 和 voice-provider.json，不进入共享设置；小牛翻译和自定义翻译服务的密钥则保存在共享设置中。普通账号功能只在登录后联网。"
      : macos
        ? "候选词翻译默认不联网，在翻译服务里选择自己的服务，或选择水杉账号把当前页的中文候选词发送到 api.msime.app 之后才会发请求；语音识别和 AI 功能仅在用户配置并启用对应服务时联网；日常拼音输入无需联网。"
        : harmony
          ? "豆包语音识别仅在用户配置并启用时联网；也可使用 HarmonyOS 系统语音识别。原始音频只在本次识别期间处理。"
          : ios
            ? "键盘扩展的日常拼音输入无需联网；账号、云同步、AI 和语音功能仅在用户启用时联网。"
            : "语音识别和 AI 联想需要自行填入 API 和 token。云候选目前支持谷歌的云接口，请注意网络问题。";
  const aboutDescription = android
    ? "为 Android 触屏输入体验打造的开放中文输入法。"
    : linux
      ? "为 Linux 桌面输入体验打造的开放中文输入法。"
      : macos
        ? "为现代 macOS 桌面体验打造的开放中文输入法。"
        : harmony
          ? mobile
            ? "为 HarmonyOS 触屏输入体验打造的开放中文输入法。"
            : "为 HarmonyOS 2-in-1 桌面输入体验打造的开放中文输入法。"
          : ios
            ? "为 iPhone 与 iPad 触屏输入体验打造的开放中文输入法。"
            : "为现代 Windows 桌面体验打造的开放中文输入法。";
  return { helpIntro, quickStart, networkDescription, aboutDescription };
}
