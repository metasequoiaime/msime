// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import type { Preferences, Snapshot, SettingsClient } from "@msime/ui";
import { useSettingsPersistence } from "../../../../packages/ui/src/settings/use-settings-persistence";

afterEach(() => {
  vi.restoreAllMocks();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("does not start a second settings load while reload is in flight", async () => {
  let hidden = false;
  vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
  const initialLoad = deferred<Snapshot>();
  const reloadLoad = deferred<Snapshot>();
  const client: SettingsClient = {
    load: vi
      .fn<SettingsClient["load"]>()
      .mockReturnValueOnce(initialLoad.promise)
      .mockReturnValue(reloadLoad.promise),
    save: vi.fn(),
  };

  const { result } = renderHook(() => {
    const mounted = useRef(true);
    const [currentSnapshot, setSnapshot] = useState<Snapshot>();
    const [draft, setDraft] = useState<Preferences>();
    return useSettingsPersistence({
      client,
      mobile: true,
      macos: false,
      mounted,
      snapshot: currentSnapshot,
      draft,
      setSnapshot,
      setDraft,
      setBusy: vi.fn(),
      setError: vi.fn(),
      setNotice: vi.fn(),
      setRecoveredBackup: vi.fn(),
      macosShuangpinKeymap: undefined,
      savedMacosShuangpinKeymap: undefined,
      setSavedMacosShuangpinKeymap: vi.fn(),
      macosWubiAutoCommitUnique: undefined,
      savedMacosWubiAutoCommitUnique: undefined,
      setSavedMacosWubiAutoCommitUnique: vi.fn(),
    });
  });

  await act(async () => {
    initialLoad.resolve(snapshot);
    await Promise.resolve();
  });
  expect(client.load).toHaveBeenCalledOnce();

  const manualReload = result.current.reload();
  expect(client.load).toHaveBeenCalledTimes(2);
  act(() => {
    hidden = true;
    document.dispatchEvent(new Event("visibilitychange"));
    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
  });

  await waitFor(() => expect(client.load).toHaveBeenCalledTimes(2));
  reloadLoad.resolve(snapshot);
  await manualReload;
});
