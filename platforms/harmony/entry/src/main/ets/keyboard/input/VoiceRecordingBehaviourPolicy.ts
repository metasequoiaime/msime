/**
 * Which of the four recording-behaviour switches apply to a given moment.
 *
 * `sound_enabled` is the master the settings page shows above the other two: 语音提示音, then 开始
 * 录音提示音 and 结束录音提示音 under it. Turning the master off silences both regardless of what
 * they say, which is the shape the page implies and the one the other hosts forward.
 *
 * The three sound switches default to on and quietening others defaults to off, matching the shared
 * schema. An absent field is an older document rather than a choice, so it takes the default rather
 * than the safest-looking value.
 */
import { HttpAsrConfigurationPolicy } from "./HttpAsrConfigurationPolicy";
import { VoiceInputConfiguration } from "./VoiceInputConfiguration";

export class VoiceRecordingBehaviourPolicy {
  static playsStartTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice) && voice.start_sound !== false;
  }

  static playsEndTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice) && voice.end_sound !== false;
  }

  /**
   * Whether a partial result should reach the panel before the recognizer is finished.
   *
   * Off unless asked for, which is the shared default. A stream of interim guesses rewriting itself
   * is useful when you want to see the recognizer working and distracting when you do not, so the
   * panel stays empty until the final result unless the user said otherwise.
   */
  static showsInterimResults(voice: VoiceInputConfiguration): boolean {
    return voice.stream_inline_preedit === true;
  }

  /**
   * 服务商是否在用户还在说话时就返回文字，正是这一点让停顿能自行结束录音。兼容 OpenAI 的上传式服务商只在录音停止后才应答，因此需要显式的停止控件；端侧模型、豆包流式和系统识别器都是流式的，与 Android 键盘内的 `LOCAL` 和 `STREAMING` 引擎一致。
   */
  static streamsPartialResults(voice: VoiceInputConfiguration): boolean {
    return !HttpAsrConfigurationPolicy.supported(voice.asr_provider);
  }

  /** Off unless asked for: taking the audio session away from whatever is playing is intrusive. */
  static quietensOthers(voice: VoiceInputConfiguration): boolean {
    return voice.mute_system_audio === true;
  }

  private static sounds(voice: VoiceInputConfiguration): boolean {
    return voice.sound_enabled !== false;
  }
}
