import * as settings from "./settings-style";
import { GroupList, Row } from "../core/platform-controls";

export interface VoiceInputIntroSectionProps {
  localVoice: boolean;
  localVoiceModelsAvailable: boolean;
  systemVoice: boolean;
  systemVoiceHostName: string;
  android: boolean;
  ios: boolean;
  macos: boolean;
  harmony: boolean;
  linux: boolean;
  showVoiceProviderSettings: boolean;
  onOpenVoice?: () => void;
}

/** Platform-specific voice entry guidance: the first group of the 语音输入 page. */
export function VoiceInputIntroSection({
  localVoice,
  localVoiceModelsAvailable,
  systemVoice,
  systemVoiceHostName,
  android,
  ios,
  macos,
  harmony,
  linux,
  showVoiceProviderSettings,
  onOpenVoice,
}: VoiceInputIntroSectionProps) {
  if (localVoice) {
    return (
      <GroupList title="本地识别">
        <p className={settings.groupNote}>
          {localVoiceModelsAvailable
            ? "录音和识别都在这台设备上完成，音频不会离开本机，也不需要任何 API Key。在下方下载一个模型并点击“使用”，保存设置后生效；下载只会连接 GitHub 或你配置的镜像。你的用户词库会作为热词提高专有名词的识别率。可选的文本润色仍会调用你配置的云服务。"
            : "录音和识别都在这台机器上完成，音频不会离开本机，也不需要任何 API Key。需要自备 whisper.cpp 的 ggml 模型文件（.bin），在下方填写它的绝对路径；模型越大越准也越慢，首次识别要等模型载入。可选的文本润色仍会调用你配置的云服务。"}
        </p>
      </GroupList>
    );
  }
  if (systemVoice) {
    return (
      <GroupList title={`${systemVoiceHostName} 系统语音`}>
        <p className={settings.groupNote}>
          保存设置后，在目标应用中启用水杉输入法，使用键盘内的语音入口录音。不需要识别 API
          Key；首次使用需授予麦克风和语音识别权限。服务可用性及是否联网由系统决定，可选文本润色仍使用你配置的云服务。
        </p>
      </GroupList>
    );
  }
  if (android) {
    return (
      <GroupList title={showVoiceProviderSettings ? "Android 语音输入" : "Android 系统语音"}>
        <p className={settings.groupNote}>
          {showVoiceProviderSettings
            ? "键盘工具栏的“语音”入口按这里配置的服务商录音并转写；没有配置可用的服务商时回退到设备自带的系统语音识别，不需要任何 API Key。识别结果会回到键盘，确认后才插入当前输入框。"
            : "从键盘工具栏的“语音”入口调用设备上的系统语音识别服务。识别结果会回到键盘，确认后才插入当前输入框。"}
        </p>
      </GroupList>
    );
  }
  if (ios) {
    return (
      <GroupList title="iOS 应用语音">
        <p className={settings.groupNote}>
          iOS
          的录音、识别和文本提交在当前共享设置与应用语音服务中完成。保存设置后，从应用内的语音入口开始；识别结果会回到当前页面，再由你确认使用。不打开无法提交到键盘扩展输入会话的
          Tauri 语音面板。
        </p>
        {onOpenVoice && (
          <Row title="应用内语音">
            <button type="button" className="secondary" onClick={onOpenVoice}>
              开始 iOS 语音
            </button>
          </Row>
        )}
      </GroupList>
    );
  }
  if (macos) {
    return (
      <GroupList title="macOS 输入法语音">
        <p className={settings.groupNote}>
          macOS
          的语音录音、云端识别和文本提交由当前输入法进程负责；保存设置后，请在目标应用中使用下方语音快捷键或输入法悬浮工具栏开始。不打开无法提交到当前输入法会话的
          Tauri 面板。
        </p>
      </GroupList>
    );
  }
  if (harmony) {
    return (
      <GroupList title="HarmonyOS 输入法语音">
        <p className={settings.groupNote}>
          豆包配置有效时，键盘直接采集 16 kHz 麦克风音频并进行实时识别；选择系统识别时由 HarmonyOS
          CoreSpeechKit 处理。识别结果会回到键盘，确认后才插入当前输入框。
        </p>
      </GroupList>
    );
  }
  return (
    <GroupList title="语音面板">
      <Row
        title="打开语音输入"
        description={linux ? "录音和识别由已配置的 provider 服务完成" : "录音和识别在本机完成"}
      >
        <button
          type="button"
          className={`secondary ${settings.openButton}`}
          disabled={!onOpenVoice}
          onClick={onOpenVoice}
        >
          打开
        </button>
      </Row>
      {linux && (
        <p className={settings.groupNote}>
          语音需要 provider 服务：录音、模型和凭据都由它负责，服务未运行时无法录音。
        </p>
      )}
    </GroupList>
  );
}
