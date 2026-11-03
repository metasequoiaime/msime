/** 视图里的 `helpcode_trigger`：辅助码前导键和它在 `editing_text` 里的字节位置。 */
export interface HelpcodeTriggerView {
  key: string;
  position: number;
}

/**
 * 辅助码前导键（偏好 `helpcode.trigger_key`）不在按键原文里（#10731）。
 *
 * 按下 `;` 之后 `editing_text` 仍是 `ni`，画原文的地方（2in1 行内「原始按键」样式、候选栏和候选窗的读音行）就看不见它，用户不知道自己是否已经进了辅助码。`preedit` 已经带着前导键，画拼写的地方不用这个。按键原文是 ASCII，字节位置就是字符下标。
 */
export class HelpcodeTriggerPolicy {
  /** 把前导键插回按键原文；字段缺失或不合法时原样返回。 */
  static text(editing: string, trigger: HelpcodeTriggerView | undefined): string {
    if (!HelpcodeTriggerPolicy.valid(editing, trigger)) return editing;
    const value: HelpcodeTriggerView = trigger as HelpcodeTriggerView;
    return editing.substring(0, value.position) + value.key + editing.substring(value.position);
  }

  /** 原文光标在插回前导键之后的文字里的位置：不在前导键前面时后移它的长度。 */
  static caret(editing: string, caret: number, trigger: HelpcodeTriggerView | undefined): number {
    if (!HelpcodeTriggerPolicy.valid(editing, trigger)) return caret;
    const value: HelpcodeTriggerView = trigger as HelpcodeTriggerView;
    return caret >= value.position ? caret + value.key.length : caret;
  }

  private static valid(editing: string, trigger: HelpcodeTriggerView | undefined): boolean {
    return (
      trigger !== undefined &&
      trigger !== null &&
      typeof trigger.key === "string" &&
      trigger.key.length > 0 &&
      Number.isInteger(trigger.position) &&
      trigger.position >= 0 &&
      trigger.position <= editing.length
    );
  }
}
