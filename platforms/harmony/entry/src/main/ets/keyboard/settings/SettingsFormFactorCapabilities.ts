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
  /** The keyboard fades and rounds only the 2in1 candidate card; the phone strip keeps its own surface and corner, so these sliders would do nothing there. */
  candidateWindowOpacity: boolean;
  candidateCornerRadius: boolean;
  inputModeHud: boolean;
  /** Key sounds, the typing melody, commit and achievement sounds: played for a hardware keyboard on a 2in1, while a phone keeps its own key feedback. */
  keySound: boolean;
  /** Background music from a music pack, streamed through AVPlayer on a 2in1 while an editor that may hear it has focus; a phone plays none. */
  music: boolean;
  /** The V, / and @ modes, claimed for the 2in1, the PC form factor they were built around: a hardware keyboard sends their digits, operators and marks through the Engine's spelling symbols. */
  pluginTriggers: boolean;
  /** The typing effect and the combo counter: a 2in1 flashes its candidate card, shows the combo and plays the tier-up sound for a hardware keyboard; a phone draws none. */
  typingEffects: boolean;
  /** 背单词书目列出单词本插件：设置页把插件目录交给背单词入口；插件只能在 2in1 的插件页安装，所以只在 2in1 声明。 */
  wordbookPacks: boolean;
  /** 表情面板的颜文字页和符号页列出符号集插件的组：插件只能在 2in1 的插件页安装，手机面板也不去读，所以只在 2in1 声明。 */
  symbolSetPacks: boolean;
}

/** Keep the shared settings page aligned with the keyboard's actual device form factor. */
export class SettingsFormFactorCapabilities {
  static resolve(deviceType: string | null | undefined): SettingsFormFactorProjection {
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
      candidateWindowOpacity: desktop,
      candidateCornerRadius: desktop,
      inputModeHud: desktop,
      keySound: desktop,
      music: desktop,
      pluginTriggers: desktop,
      typingEffects: desktop,
      wordbookPacks: desktop,
      symbolSetPacks: desktop,
    };
  }
}
