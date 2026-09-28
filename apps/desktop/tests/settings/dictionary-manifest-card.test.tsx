// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DictionaryManifestCard } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the packaged profile and abbreviated source revision", async () => {
  render(
    <DictionaryManifestCard
      read={async () => ({
        profile: "desktop-standard",
        sourceCommit: "1234567890abcdef",
      })}
    />,
  );

  expect(await screen.findByText("desktop-standard")).toBeTruthy();
  expect(screen.getByText("1234567890ab")).toBeTruthy();
});

test("reports a manifest read failure", async () => {
  render(<DictionaryManifestCard read={async () => Promise.reject(new Error("synthetic"))} />);

  expect(await screen.findByText("无法读取随应用安装的词库清单，请重新安装后再试。")).toBeTruthy();
});

test("does not render stale results after unmount", async () => {
  let resolve: ((value: { profile: string; sourceCommit: string }) => void) | undefined;
  const read = () =>
    new Promise<{ profile: string; sourceCommit: string }>((done) => {
      resolve = done;
    });
  const { unmount } = render(<DictionaryManifestCard read={read} />);
  unmount();
  resolve?.({ profile: "unmounted", sourceCommit: "1234567890abcdef" });
  await Promise.resolve();
  expect(screen.queryByText("unmounted")).toBeNull();
});
