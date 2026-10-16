/**
 * The form-factor boundary for the Harmony input-method panel.
 *
 * A 2-in-1 has physical keys and therefore gets a candidate window plus an optional toolbar.
 * Phones and tablets get the touch keyboard. Keep this decision independent of the device-info
 * singleton so the most important product distinction can be tested without a device.
 */
export class KeyboardFormFactorPolicy {
  static isDesktop(deviceType: string | null | undefined): boolean {
    return deviceType === "2in1";
  }

  /** 手机横屏：屏幕宽大于高，且最短边不到 600vp。平板和折叠屏的内屏不算，界线与 Android 的 `smallestScreenWidthDp` 相同。 */
  static phoneLandscape(widthVp: number, heightVp: number): boolean {
    return widthVp > heightVp && Math.min(widthVp, heightVp) < 600;
  }
}
