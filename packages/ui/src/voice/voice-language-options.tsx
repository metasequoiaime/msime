import catalog from "./voice-language-catalog.json";

export interface VoiceLanguageOptionsProps {
  /** Native system recognizers use locale identifiers such as `zh-CN`; provider recognizers use the short forms. */
  systemVoice?: boolean;
}

/** Shared language choices for voice input settings and the native voice panel. */
export function VoiceLanguageOptions({ systemVoice = false }: VoiceLanguageOptionsProps) {
  return (
    <>
      {catalog
        .filter((choice) => (systemVoice ? choice.system_value : choice.provider_value))
        .map((choice) => (
          <option key={choice.id} value={systemVoice ? choice.system_value : choice.provider_value}>
            {systemVoice ? choice.system_title : choice.provider_title}
          </option>
        ))}
    </>
  );
}

/** Display a stored language code in the same names used by the voice settings choices. */
export function voiceLanguageTitle(language: string | undefined): string {
  const code = (language ?? "").trim();
  // 目录中的 mobile_title 也覆盖当前移动端未提供选择项的语言（例如日语），
  // 因而已保存的旧值仍能显示本地化标题；未知值继续由 default 原样回显。
  const mobileTitle = (id: string) =>
    catalog.find((choice) => choice.id === id)?.mobile_title ?? id;
  switch (code.toLowerCase()) {
    case "":
    case "zh":
    case "zh-cn":
      return mobileTitle("zh-cn");
    case "yue":
    case "zh-hk":
      return mobileTitle("yue");
    case "en":
    case "en-us":
      return mobileTitle("en");
    case "ja":
    case "ja-jp":
      return mobileTitle("ja");
    case "auto":
      return mobileTitle("auto");
    default:
      return code;
  }
}

/** 手机语音页提供的识别语言，按设计稿的顺序。它们是 Android 的 `VoicePage` 写入 `voice_input.language` 的值。 */
export type MobileVoiceLanguage = Exclude<(typeof catalog)[number]["mobile_value"], "">;

export const MOBILE_VOICE_LANGUAGES: readonly { value: MobileVoiceLanguage; label: string }[] =
  catalog
    .filter((choice) => choice.mobile_value)
    .map((choice) => ({
      value: choice.mobile_value as MobileVoiceLanguage,
      label: choice.mobile_title,
    }));

/** 存储的 `voice_input.language` 对应哪个提供的语言。忽略大小写，所以交给系统识别器的区域形式 `zh-CN` 读作普通话；空值也是普通话，因为每个识别器都回退到普通话。其他值为 null：页面原样显示，而不是假装它是四种之一。 */
export function mobileVoiceLanguageOf(value: string): MobileVoiceLanguage | null {
  const normalized = value.trim().toLowerCase();
  if (normalized.length === 0) return "zh-cn";
  return MOBILE_VOICE_LANGUAGES.find((language) => language.value === normalized)?.value ?? null;
}

/** 识别器能否支持某种语言。鸿蒙系统识别器（CoreSpeechKit）只识别普通话；云端、HTTP 和本地识别器接受或自动检测全部四种。 */
export function mobileVoiceLanguageSupported(
  language: MobileVoiceLanguage,
  systemVoice: boolean,
): boolean {
  return !systemVoice || language === "zh-cn";
}

/** 某个选择写入的值：系统识别器把 `voice_input.language` 当作区域设置，所以对它把普通话存为 `zh-CN`；其他识别器都用短形式。 */
export function mobileVoiceLanguageValue(language: MobileVoiceLanguage, systemVoice: boolean) {
  return systemVoice && language === "zh-cn" ? "zh-CN" : language;
}
