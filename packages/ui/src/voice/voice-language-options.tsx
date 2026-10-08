export interface VoiceLanguageOptionsProps {
  /** Native system recognizers use locale identifiers such as `zh-CN`; provider recognizers use the short forms. */
  systemVoice?: boolean;
}

/** Shared language choices for voice input settings and the native voice panel. */
export function VoiceLanguageOptions({ systemVoice = false }: VoiceLanguageOptionsProps) {
  return (
    <>
      <option value={systemVoice ? "zh-CN" : "zh-cn"}>中文（普通话）</option>
      <option value={systemVoice ? "en-US" : "en"}>English</option>
      <option value={systemVoice ? "ja-JP" : "ja"}>日本語</option>
      {!systemVoice && <option value="auto">自动识别</option>}
    </>
  );
}

/** 手机语音页提供的识别语言，按设计稿的顺序。它们是 Android 的 `VoicePage` 写入 `voice_input.language` 的值。 */
export const MOBILE_VOICE_LANGUAGES = [
  { value: "zh-cn", label: "普通话" },
  { value: "yue", label: "粤语" },
  { value: "en", label: "英语" },
  { value: "auto", label: "普通话 + 英语" },
] as const;

export type MobileVoiceLanguage = (typeof MOBILE_VOICE_LANGUAGES)[number]["value"];

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
