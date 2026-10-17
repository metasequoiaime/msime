// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import type { HostCapabilities } from "../../../../packages/ui/src/core/host-capabilities";
import { panelSurfaceTheme, useHostPlatform, usePanelHost } from "../../src/input/panel-host";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: vi.fn(() => false),
  invoke: vi.fn(),
}));

function makeHost(platform: HostCapabilities["platform"], surface_themes_follow?: boolean) {
  return { platform, surface_themes_follow } as HostCapabilities;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

test("panel surface theme waits for host capabilities and follows the host policy", () => {
  const surface = "dark" as const;
  expect(panelSurfaceTheme({ host: undefined, resolved: false }, surface)).toBeUndefined();
  expect(
    panelSurfaceTheme({ host: makeHost("macos", true), resolved: true }, surface),
  ).toBeUndefined();
  expect(panelSurfaceTheme({ host: makeHost("android", false), resolved: true }, surface)).toBe(
    surface,
  );
});

test("panel host module exposes the discovered platform to callers", async () => {
  const windowsHost = makeHost("windows");

  function Probe() {
    const host = usePanelHost(undefined);
    const platform = useHostPlatform(windowsHost);
    return <output>{`${host.resolved}:${platform ?? "none"}`}</output>;
  }

  render(<Probe />);
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("true:windows"));
  expect(isTauri).toHaveBeenCalledOnce();
  expect(invoke).not.toHaveBeenCalled();
});
