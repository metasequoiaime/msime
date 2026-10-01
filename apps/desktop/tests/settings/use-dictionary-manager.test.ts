// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  useDictionaryManager,
  type DictionaryManagerClient,
} from "../../../../packages/ui/src/settings/use-dictionary-manager";

test("ignores a second all-dictionaries export while the first is pending", async () => {
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const list = vi.fn(async (_offset: number, _limit: number, kind?: string) => {
    await pending;
    return {
      entries:
        kind === "pinyin"
          ? [{ kind: "pinyin" as const, key: "nihao", value: "你好", weight: 10000 }]
          : [],
      has_more: false,
    };
  });
  const saveExport = vi.fn().mockResolvedValue("/synthetic/export.txt");
  const client: DictionaryManagerClient = {
    dictionary: { list, edit: vi.fn().mockResolvedValue(undefined) },
    saveExport,
  };
  const { result } = renderHook(() =>
    useDictionaryManager({ client, confirm: vi.fn().mockResolvedValue(true) }),
  );

  let first!: Promise<void>;
  act(() => {
    first = result.current.exportAllPhrases();
  });
  await waitFor(() => expect(result.current.phraseBusy).toBe(true));

  let second!: Promise<void>;
  act(() => {
    second = result.current.exportAllPhrases();
  });
  expect(list).toHaveBeenCalledTimes(1);

  resolve();
  await act(async () => {
    await first;
    await second;
  });
  expect(saveExport).toHaveBeenCalledOnce();
  expect(result.current.phraseBusy).toBe(false);
});
