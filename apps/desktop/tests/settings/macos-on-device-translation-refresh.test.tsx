// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { test, expect, vi } from "vitest";
import type { SettingsClient } from "@msime/ui";
import { useMacosSettings } from "../../../../packages/ui/src/settings/use-macos-settings";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("旧的系统翻译语言读取不能覆盖窗口聚焦后的结果", async () => {
  const first = deferred<string[]>();
  const second = deferred<string[]>();
  const downloadableLanguages = vi
    .fn()
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  const client = {
    onDeviceTranslation: { downloadableLanguages },
  } as unknown as Pick<
    SettingsClient,
    "inputSourceStartup" | "onDeviceTranslation" | "loadMacosShuangpinKeymap"
  >;

  function Page() {
    const { onDeviceDownloadable } = useMacosSettings({ client, macos: true, setError: () => {} });
    return <output>{onDeviceDownloadable.join(",")}</output>;
  }

  render(<Page />);
  await waitFor(() => expect(downloadableLanguages).toHaveBeenCalledTimes(1));
  fireEvent.focus(window);
  await waitFor(() => expect(downloadableLanguages).toHaveBeenCalledTimes(2));

  await act(async () => {
    second.resolve(["ja"]);
  });
  expect(screen.getByRole("status").textContent).toBe("ja");
  await act(async () => {
    first.resolve(["en"]);
  });
  expect(screen.getByRole("status").textContent).toBe("ja");
});
