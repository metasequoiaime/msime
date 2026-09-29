import { expect, test } from "vitest";
import { diagnosticLogPreferences } from "../../../../packages/ui/src/settings/diagnostic-log-preferences";

test("fills missing diagnostic log switches with disabled defaults", () => {
  expect(diagnosticLogPreferences()).toEqual({ server: false, tsf: false });
  expect(diagnosticLogPreferences({ diagnostic_log: { server: true } })).toEqual({
    server: true,
    tsf: false,
  });
});
