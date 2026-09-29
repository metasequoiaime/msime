import { expect, test } from "vitest";
import { defaultFloatingToolbar } from "../../../../packages/ui/src/settings/floating-toolbar-defaults";
import { floatingToolbarPreferences } from "../../../../packages/ui/src/settings/floating-toolbar-preferences";

test("fills missing floating toolbar fields from the shared defaults", () => {
  expect(floatingToolbarPreferences()).toEqual(defaultFloatingToolbar);
  expect(
    floatingToolbarPreferences({ floating_toolbar: { enabled: false, font_size: 20 } }),
  ).toMatchObject({ ...defaultFloatingToolbar, enabled: false, font_size: 20 });
});
