import { KeyboardFormFactorPolicy } from "../KeyboardFormFactorPolicy";

/** The settings capabilities that exist only with HarmonyOS's desktop input surface. */
export interface SettingsFormFactorProjection {
  mobileSettings: boolean;
  panelWindows: boolean;
  floatingToolbar: boolean;
  floatingToolbarAppearance: boolean;
  floatingToolbarComponents: boolean;
  /** The pad and voice buttons exist only on the 2in1 toolbar, so only there is switching them off an outcome. */
  floatingToolbarHandwriting: boolean;
  floatingToolbarVoice: boolean;
  /** Mode-switch chords, number-row selection and the voice hotkeys are routed on every device once a keyboard is attached, so their switches are offered on every device too: hiding them on a phone left behaviour the owner could not turn off. */
  modeSwitchShortcuts: boolean;
  panelShortcuts: boolean;
  numberRowSelection: boolean;
  voiceHotkeys: boolean;
  /** A phone draws its candidates as one horizontal strip whatever the preference says; only the 2in1 candidate window can be a vertical list. */
  fixedCandidateLayout: "horizontal" | null;
  candidateFollowCursor: boolean;
  /** 「候选栏大小」（`candidate_scale_percent`）：手机和平板的键盘按它把候选栏和跟随候选栏的工具栏一起缩放、按键不动（#10137），所以只在这两种形态声明；2in1 的候选卡片只随字号调整，平台层面的能力位保持关闭。 */
  candidateWindowScale: boolean;
  /** The keyboard fades and rounds only the 2in1 candidate card; the phone strip keeps its own surface and corner, so these sliders would do nothing there. */
  candidateWindowOpacity: boolean;
  candidateCornerRadius: boolean;
  inputModeHud: boolean;
  /** Key sounds, the typing melody, commit and achievement sounds: played for a hardware keyboard on a 2in1, while a phone keeps its own key feedback. */
  keySound: boolean;
  /** Background music from a music pack, streamed through AVPlayer on a 2in1 while an editor that may hear it has focus; a phone plays none. */
  music: boolean;
  /** V、/ 和 @ 三种模式（`plugin_triggers`）：只在 2in1 上声明。设置页靠这一项给出 / 和 @ 的开关、@ 名单和插件页的指令表、短语表。手机和平板不提供快捷模式入口：设置页没有「快捷模式」组，功能面板也没有「本地输入」，K 和 / 模式无从进入，所以插件页不列指令表、短语表，也不给 @ 名单。 */
  pluginTriggers: boolean;
  /** The typing effect and the combo counter: a 2in1 flashes its candidate card, shows the combo and plays the tier-up sound for a hardware keyboard; a phone draws none. */
  typingEffects: boolean;
  /** 背单词书目列出单词本插件：设置页把插件目录交给背单词入口。手机、平板和 2in1 都有插件页，每种形态都声明。 */
  wordbookPacks: boolean;
  /** 表情面板的颜文字页和符号页列出符号集插件的组：键盘每种形态都读镜像过来的插件目录，每种形态都声明。 */
  symbolSetPacks: boolean;
  /** 工具栏的显示方式和工具开关（`harmony_toolbar`）：只有手机键盘画那一行工具栏，2in1 的键盘没有它，所以只在手机上声明。 */
  touchToolbarTools: boolean;
  /** 屏幕键盘页「按键反馈」组的「音效」选择（`touch_key_sound_packs`）：手机和平板的触屏键盘按 `plugins.key_sound.pack` 发按键音，所以只在这两种形态声明；2in1 的音效包在插件页里选，它的屏幕键盘页也没有这一组。 */
  touchKeySoundPacks: boolean;
  /** 触屏设置里的插件页（`touch_plugin_packs`）：手机和平板的设置页列出插件页，只列触屏键盘用得上的数据插件，导入只收 .zip（这两种形态的文件选择器不能选文件夹）；2in1 走桌面形态的插件页，不声明。 */
  touchPluginPacks: boolean;
  /** 皮肤页的「制作皮肤」标签：编辑器是桌面形态的设置界面，只在 2in1 上声明；手机的皮肤页保持原样。 */
  candidateSkinEditor: boolean;
  /** 屏幕键盘页「大屏单手时侧边显示数字键盘」（`touch_one_handed_side_panel`，#10203）：只在有大屏触屏键盘的设备上声明，即平板和可折叠手机（展开后是大屏）；直板手机的单手模式铺满整行，空不出一侧，2in1 不画触摸按键。 */
  oneHandedSidePanel: boolean;
  /** 屏幕键盘页「分离式键盘」四行（`touch_keyboard_split` 等，#10224）：与 `oneHandedSidePanel` 一样只在有大屏触屏键盘的设备上声明，即平板和可折叠手机；直板手机的最短边不到最低的 480vp 启用宽度，从不分离，2in1 不画触摸按键。 */
  splitKeyboard: boolean;
  /** 语音页「本地停顿时间」和「判停灵敏度」（`voice_pause_settings`，#10310）：手机和平板的触屏键盘在点按开始的本地识别里按停顿结束录音，所以在这两种形态声明；2in1 的语音面板由用户或快捷键结束，不按停顿结束。 */
  voicePauseSettings: boolean;
}

/** Keep the shared settings page aligned with the keyboard's actual device form factor. */
export class SettingsFormFactorCapabilities {
  /** `foldable` 是设备能不能折叠（`display.isFoldable()`），只影响手机上的 `oneHandedSidePanel`。 */
  static resolve(
    deviceType: string | null | undefined,
    foldable: boolean = false,
  ): SettingsFormFactorProjection {
    const desktop: boolean = KeyboardFormFactorPolicy.isDesktop(deviceType);
    return {
      mobileSettings: !desktop,
      panelWindows: desktop,
      floatingToolbar: desktop,
      floatingToolbarAppearance: desktop,
      floatingToolbarComponents: desktop,
      floatingToolbarHandwriting: desktop,
      floatingToolbarVoice: desktop,
      modeSwitchShortcuts: true,
      panelShortcuts: desktop,
      numberRowSelection: true,
      voiceHotkeys: true,
      fixedCandidateLayout: desktop ? null : "horizontal",
      candidateFollowCursor: desktop,
      candidateWindowScale: !desktop,
      candidateWindowOpacity: desktop,
      candidateCornerRadius: desktop,
      inputModeHud: desktop,
      keySound: desktop,
      music: desktop,
      pluginTriggers: desktop,
      typingEffects: desktop,
      wordbookPacks: true,
      symbolSetPacks: true,
      touchToolbarTools: !desktop,
      touchKeySoundPacks: !desktop,
      touchPluginPacks: !desktop,
      candidateSkinEditor: desktop,
      oneHandedSidePanel: deviceType === "tablet" || (deviceType === "phone" && foldable),
      splitKeyboard: deviceType === "tablet" || (deviceType === "phone" && foldable),
      voicePauseSettings: !desktop,
    };
  }
}
