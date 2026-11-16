import { expect, test } from "vitest";

interface Capability {
  windows: string[];
  permissions: string[];
}
const capabilities = Object.values(
  import.meta.glob<Capability>("../../src-tauri/capabilities/*.json", {
    eager: true,
    import: "default",
  }),
);
function permissionsFor(window: string) {
  return new Set(
    capabilities
      .filter((capability) => capability.windows.includes(window))
      .flatMap((capability) => capability.permissions),
  );
}

test("独立剪贴板窗口有拖动权限，保持不可缩放且不授权其他窗口", () => {
  const permissions = permissionsFor("clipboard-panel");
  expect(permissions.has("core:window:allow-start-dragging")).toBe(true);
  expect(permissions.has("core:window:allow-start-resize-dragging")).toBe(false);
  expect(permissionsFor("unknown-panel").has("core:window:allow-start-dragging")).toBe(false);
});

// These windows subscribe to preferences-changed; voice-panel also subscribes to
// voice-update. Both listen and unlisten are required for the subscription lifecycle.
test.each(["keyboard-panel", "handwriting-panel", "voice-panel", "emoji-panel"])(
  "%s can subscribe to host updates and clean up its listeners",
  (window) => {
    const permissions = permissionsFor(window);
    expect(permissions.has("core:event:allow-listen")).toBe(true);
    expect(permissions.has("core:event:allow-unlisten")).toBe(true);
    expect(permissions.has("core:event:allow-emit")).toBe(false);
    expect(permissions.has("core:event:allow-emit-to")).toBe(false);
    expect(permissions.has("core:default")).toBe(false);
  },
);

test.each(["cloud-clipboard-panel", "cloud-dictionary-panel", "unknown-panel"])(
  "%s does not inherit input panel event permissions",
  (window) => {
    expect(permissionsFor(window).has("core:event:allow-listen")).toBe(false);
    expect(permissionsFor(window).has("core:event:allow-unlisten")).toBe(false);
  },
);
