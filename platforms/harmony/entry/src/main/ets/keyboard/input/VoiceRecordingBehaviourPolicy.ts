/**
 * 录音行为的几个开关在某一刻是否生效。
 *
 * 提示音只认总开关 `sound_enabled`（设置页的「提示音」）：开始、结束提示音都跟着它。存下的 `start_sound`、`end_sound` 不再读，设置页也不再给这两个开关（`voice_cue_switches` 为 false），与 macOS、Windows 一致；以前单独关掉其中一个的人会重新听到它，这是有意的，不留一个看不见却仍然生效的值。
 *
 * 提示音默认开，静音其他声音默认关，与共享 schema 一致。字段缺失说明是旧文档而不是用户的选择，所以取默认值，而不是看起来最安全的值。
 */
import { DoubaoCredentialPolicy, VoiceInputConfiguration } from "./VoiceInputConfiguration";
import { VoiceRecognitionPolicy, VOICE_PAUSE_DEFAULT_MS } from "./VoiceRecognitionPolicy";

export class VoiceRecordingBehaviourPolicy {
  static playsStartTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice);
  }

  static playsEndTone(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.sounds(voice);
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
   * 这次聆听实际交给系统识别（Core Speech Kit）：选了系统识别、服务为空，或者豆包一项凭据都没填（出厂配置，`DoubaoCredentialPolicy.attempted` 为假），与 `HarmonyVoiceRecognizer.start` 的分派一致。
   */
  static usesSystemRecognizer(voice: VoiceInputConfiguration): boolean {
    if (voice.asr_provider === "doubao") return !DoubaoCredentialPolicy.attempted(voice);
    return voice.asr_provider === "system" || voice.asr_provider.length === 0;
  }

  /**
   * 键盘自己的停顿计时是否结束这次录音：本地模型和系统识别（#10310）。豆包流式不再由停顿结束，与 Android #9886 之后的 `ImeVoiceEntry.heard(..., pauseEnds)` 一致：它不报是否在说话，整句流式说话期间要到音频超过 15 秒才回第一段文字，按出字后停顿计时会在用户还在说时掐断长句。系统识别照旧由键盘计时：自 #5792 起这就是触屏上点按开始的系统识别唯一确定的结束方式，写音频模式下 Core Speech Kit 会不会自己判停没有在设备上确认过，出厂配置走的正是这条路，不能改。
   */
  static keyboardStopsOnPause(voice: VoiceInputConfiguration): boolean {
    return (
      voice.asr_provider === "local" || VoiceRecordingBehaviourPolicy.usesSystemRecognizer(voice)
    );
  }

  /**
   * 按停顿结束时等多久：本地识别用设置页的「本地停顿时间」（`voice_input.pause_millis`），系统识别沿用原来固定的 1500 毫秒，设置页写明这两项只管本地识别。
   */
  static pauseMillis(voice: VoiceInputConfiguration): number {
    return voice.asr_provider === "local"
      ? VoiceRecognitionPolicy.pauseMillis(voice.pause_millis)
      : VOICE_PAUSE_DEFAULT_MS;
  }

  /**
   * 交给本地识别的判停灵敏度（`LocalAsrRequest.stopSensitivity`）：只有按停顿结束的本地识别用存下的值，其余（按住说话、快捷键、其他服务）为 0，worker 沿用缺省的 VAD 阈值。缺键时也为 0，与默认的 50 换算出同一个阈值。
   */
  static stopSensitivity(voice: VoiceInputConfiguration, pauseEnds: boolean): number {
    return pauseEnds && voice.asr_provider === "local" ? (voice.stop_sensitivity ?? 0) : 0;
  }

  /**
   * 停顿之后录音会不会自己结束，决定聆听面板提示「停顿后自动上屏」还是「说完后点「说完了」」：本地模型和系统识别由键盘计时；豆包（填了凭据时）和上传式服务商都要用户结束。
   */
  static endsOnPause(voice: VoiceInputConfiguration): boolean {
    return VoiceRecordingBehaviourPolicy.keyboardStopsOnPause(voice);
  }

  /** Off unless asked for: taking the audio session away from whatever is playing is intrusive. */
  static quietensOthers(voice: VoiceInputConfiguration): boolean {
    return voice.mute_system_audio === true;
  }

  private static sounds(voice: VoiceInputConfiguration): boolean {
    return voice.sound_enabled !== false;
  }
}
