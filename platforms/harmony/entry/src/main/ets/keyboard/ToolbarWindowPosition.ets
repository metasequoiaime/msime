import window from '@ohos.window';

/** 新拖动开始前读取面板实际位置，单位是像素。 */
export function toolbarWindowPosition(name: string | undefined): [number, number] | undefined {
  if (name === undefined) return undefined;
  try {
    const rect: window.Rect = window.findWindow(name).getWindowProperties().windowRect;
    if (Number.isFinite(rect.left) && Number.isFinite(rect.top)) return [rect.left, rect.top];
  } catch (_) {}
  return undefined;
}
