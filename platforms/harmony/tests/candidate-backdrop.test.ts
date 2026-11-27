import { CandidateDecorationLayout } from "../entry/src/main/ets/keyboard/candidate/CandidateDecorationLayout";
import { DesktopSurface } from "../entry/src/main/ets/keyboard/DesktopSurface";

function visible(desktop: boolean, surface: DesktopSurface): boolean {
  return CandidateDecorationLayout.keyboardBackdropVisible(desktop, surface === DesktopSurface.NONE);
}
function check(value: boolean, label: string): void {
  if (!value) throw new Error(label);
}

check(!visible(true, DesktopSurface.NONE), "电脑候选窗不透出键盘背景与照片");
for (const surface of [DesktopSurface.EMOJI, DesktopSurface.VOICE, DesktopSurface.HANDWRITING,
  DesktopSurface.SCREEN_KEYBOARD]) {
  check(visible(true, surface), "电脑工具面板仍保留键盘背景");
}
check(visible(false, DesktopSurface.NONE), "手机键盘保留背景与照片");
check(visible(false, DesktopSurface.EMOJI), "手机面板保留背景");
console.log("Harmony candidate backdrop visibility passed");
