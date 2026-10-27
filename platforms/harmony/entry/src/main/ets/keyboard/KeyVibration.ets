import vibrator from '@ohos.vibrator';

/**
 * 按键振动在 TaskPool 线程上发出的那一次 `startVibration`。
 *
 * `startVibration` 虽然返回 Promise，NAPI 层却是先在调用线程上同步把请求发给振动服务再返回；放在键盘的 UI 线程上，每按一下键都要等这次 IPC，开了振动后快速打字跟不上手。单独成文件、只引入 `@ohos.vibrator`，跑它的 worker 不用加载键盘的其他模块。
 *
 * `effectId` 为空时按 `millis` 毫秒的时长振动，否则播这个预置效果；`intensity` 为 0 表示不指定强度。`followsSystem` 的含义见 `HapticPlan`。
 */
@Concurrent
export async function startKeyVibration(effectId: string, intensity: number, millis: number,
  followsSystem: boolean): Promise<void> {
  const attribute: vibrator.VibrateAttribute = { usage: followsSystem ? 'touch' : 'physicalFeedback' };
  // 失败照原样抛回调用方：`KeyboardSession.vibrate` 据此退回 UI 线程，worker 里不记日志，免得为此加载日志模块。
  try {
    if (effectId.length === 0) {
      const timed: vibrator.VibrateTime = { type: 'time', duration: millis };
      await vibrator.startVibration(timed, attribute);
      return;
    }
    const preset: vibrator.VibratePreset = { type: 'preset', effectId: effectId, count: 1 };
    if (intensity > 0) {
      preset.intensity = intensity;
    }
    await vibrator.startVibration(preset, attribute);
  } catch (error) {
    throw error as Error;
  }
}
