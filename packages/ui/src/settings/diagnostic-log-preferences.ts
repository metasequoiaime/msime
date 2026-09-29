import type { Preferences } from "../index";
import type { DiagnosticLogPreferences } from "./diagnostic-logs-section";

export type DiagnosticLogPreferencesSource = Pick<Preferences, "diagnostic_log">;

/** Resolves the effective diagnostic log switches for the developer settings page. */
export function diagnosticLogPreferences(
  draft?: DiagnosticLogPreferencesSource,
): DiagnosticLogPreferences {
  return {
    server: draft?.diagnostic_log?.server ?? false,
    tsf: draft?.diagnostic_log?.tsf ?? false,
  };
}
