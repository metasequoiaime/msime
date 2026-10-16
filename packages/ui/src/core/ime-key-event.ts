/** 识别 React 键盘事件中的组词标记与 IME 首尾按键兼容码，不改变事件默认行为。 */
export function isComposingKeyEvent(event: {
  nativeEvent: { isComposing: boolean };
  keyCode: number;
}): boolean {
  return event.nativeEvent.isComposing || event.keyCode === 229;
}
