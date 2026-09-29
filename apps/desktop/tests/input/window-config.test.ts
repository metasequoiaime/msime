import { expect, test } from "vitest";
import base from "../../src-tauri/tauri.conf.json";
import windows from "../../src-tauri/tauri.windows.conf.json";
import linux from "../../src-tauri/tauri.linux.conf.json";
import macos from "../../src-tauri/tauri.macos.conf.json";
import capability from "../../src-tauri/capabilities/default.json";

test("shared settings uses the canonical client identifier", () => {
  expect(base.identifier).toBe("app.msime.client");
});

test("Windows custom titlebar disables native decorations without losing window constraints", () => {
  // Platform config replaces the windows array rather than merging its items.
  expect(windows.app.windows).toEqual(
    base.app.windows.map((window: object) => ({ ...window, decorations: false })),
  );
  expect(base.app.windows[0]).not.toHaveProperty("decorations");
  expect(windows).not.toHaveProperty("build");
  expect(windows.app).not.toHaveProperty("security");
  expect(capability.windows).toContain("main");
  for (const action of [
    "start-dragging",
    "start-resize-dragging",
    "minimize",
    "maximize",
    "unmaximize",
    "close",
  ])
    expect(capability.permissions).toContain(`core:window:allow-${action}`);
});

test("GNOME headerbar replaces the native Linux titlebar with the same window constraints", () => {
  // The page draws the headerbar and the resize edges, as it does on Windows.
  expect(linux.app.windows).toEqual(
    base.app.windows.map((window: object) => ({ ...window, decorations: false })),
  );
  expect(linux).not.toHaveProperty("build");
  expect(linux).not.toHaveProperty("bundle");
  expect(linux.app).not.toHaveProperty("security");
});

test("macOS overlays its traffic lights on the sidebar and keeps the native frame", () => {
  // Native decorations stay on: the frame still resizes, rounds and shadows the window; only the title bar becomes transparent so the sidebar and the page toolbar run to the top edge.
  expect(macos.app.windows).toEqual(
    base.app.windows.map((window: object) => ({
      ...window,
      titleBarStyle: "Overlay",
      hiddenTitle: true,
      trafficLightPosition: { x: 18, y: 24 },
    })),
  );
  expect(macos.app.windows[0]).not.toHaveProperty("decorations");
  expect(macos).not.toHaveProperty("build");
  expect(macos.app).not.toHaveProperty("security");
  // The bundle section the macOS package depends on is untouched by the window entry.
  expect(macos.bundle.active).toBe(true);
});
