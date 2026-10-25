import { expect, test } from "vitest";
import {
  commandLineCandidatesEnabled,
  diagnosticLogPreferences,
} from "../../../../packages/ui/src/settings/diagnostic-log-preferences";

test("fills missing diagnostic log switches with disabled defaults", () => {
  expect(diagnosticLogPreferences()).toEqual({ server: false, tsf: false });
  expect(diagnosticLogPreferences({ diagnostic_log: { server: true } })).toEqual({
    server: true,
    tsf: false,
  });
});

test("command line candidate lookup stays off until the user turns it on", () => {
  // 没写过这个键的配置（包括只开了诊断日志的）都不能让命令行读到词库。
  expect(commandLineCandidatesEnabled()).toBe(false);
  expect(commandLineCandidatesEnabled({ diagnostic_log: { server: true, tsf: true } })).toBe(false);
  expect(commandLineCandidatesEnabled({ diagnostic_log: { command_line_candidates: true } })).toBe(
    true,
  );
  // 诊断日志开关的取值不受这一项影响。
  expect(diagnosticLogPreferences({ diagnostic_log: { command_line_candidates: true } })).toEqual({
    server: false,
    tsf: false,
  });
});
