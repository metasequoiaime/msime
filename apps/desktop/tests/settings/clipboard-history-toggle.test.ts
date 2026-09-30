// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useClipboardHistoryToggle, type Preferences } from "@msime/ui";

const draft = {
  scheme: "quanpin",
  clipboard_history: true,
} as unknown as Preferences;

test("writes clipboard history into the latest draft", () => {
  const setDraft = vi.fn();
  const { result } = renderHook(() =>
    useClipboardHistoryToggle({
      draft,
      enabled: true,
      setDraft,
      setError: vi.fn(),
    }),
  );

  act(() => result.current(false));

  const updater = setDraft.mock.calls[0][0] as (value: Preferences) => Preferences;
  expect(updater({ ...draft, candidate_page_size: 9 })).toMatchObject({
    candidate_page_size: 9,
    clipboard_history: false,
  });
});
