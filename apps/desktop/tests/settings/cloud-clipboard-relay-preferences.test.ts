import { expectTypeOf, test } from "vitest";
import type { Preferences } from "@msime/ui";
import type { CloudClipboardRelayPreferences } from "../../../../packages/ui/src/settings/cloud-clipboard-relay-preferences";

test("云剪贴板中继公共偏好与总偏好保持一致", () => {
  expectTypeOf<CloudClipboardRelayPreferences>().toEqualTypeOf<
    NonNullable<Preferences["cloud_clipboard_relay"]>
  >();
});
