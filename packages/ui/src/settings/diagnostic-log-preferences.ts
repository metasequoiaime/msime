import type { Preferences } from "../core/preferences-model";
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

/** 「命令行查询候选」开关的当前值；没写过这个键的配置一律按关闭处理，不让命令行在用户没同意时读到词库。 */
export function commandLineCandidatesEnabled(draft?: DiagnosticLogPreferencesSource): boolean {
  return draft?.diagnostic_log?.command_line_candidates ?? false;
}
