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
