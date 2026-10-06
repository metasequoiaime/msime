/**
 * Bounds transient Engine diagnostics before they reach the keyboard surface, ported from
 * platforms/android/java/app/msime/android/InputDiagnosticPolicy.java.
 */
export class InputDiagnosticPolicy {
  static readonly DISMISS_DELAY_MILLIS: number = 4000;
  static readonly MAX_LENGTH: number = 1024;

  static normalize(value: string | null): string {
    if (value === null) {
      return "";
    }
    const normalized: string = value.trim();
    if (normalized.length === 0) {
      return "";
    }
    if (normalized.length <= InputDiagnosticPolicy.MAX_LENGTH) {
      return normalized;
    }
    let end: number = InputDiagnosticPolicy.MAX_LENGTH - 1;
    // 避免截断 emoji 时把孤立的高代理项带入诊断提示。
    if (
      end > 0 &&
      normalized.charCodeAt(end - 1) >= 0xd800 &&
      normalized.charCodeAt(end - 1) <= 0xdbff
    ) {
      end -= 1;
    }
    return normalized.substring(0, end) + "…";
  }

  static visible(value: string | null): boolean {
    return InputDiagnosticPolicy.normalize(value).length > 0;
  }
}
