// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useMobileSettingsSubview } from "../../../../packages/ui/src/settings/use-mobile-settings-subview";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState(null, "");
});

function useDictionaryDetail() {
  return useMobileSettingsSubview<string>({
    mobile: true,
    stateKey: "dictionaryDetail",
    decode: (state, currentValue) =>
      typeof state?.dictionaryDetail === "string" &&
      (currentValue === null || state.dictionaryDetail === currentValue)
        ? state.dictionaryDetail
        : null,
    encode: (value) => ({ page: "dictionary", dictionaryDetail: value }),
  });
}

test("opens a subview in the settings history and closes it with browser back", () => {
  window.history.replaceState({ page: "dictionary" }, "");
  const { result } = renderHook(useDictionaryDetail);
  const back = vi.spyOn(window.history, "back").mockImplementation(() => undefined);

  act(() => result.current.open("work"));

  expect(result.current.value).toBe("work");
  expect(window.history.state).toEqual({
    page: "dictionary",
    msimeSettings: true,
    dictionaryDetail: "work",
  });

  act(() => result.current.close());
  expect(back).toHaveBeenCalledOnce();
});

test("follows popstate and clears a subview when the route no longer names it", () => {
  const { result } = renderHook(useDictionaryDetail);
  act(() => result.current.open("work"));

  act(() => {
    window.dispatchEvent(
      new PopStateEvent("popstate", { state: { msimeSettings: true, page: "dictionary" } }),
    );
  });

  expect(result.current.value).toBeNull();
});

test("does not replace an open detail with a different route value", () => {
  const { result } = renderHook(useDictionaryDetail);
  act(() => result.current.open("work"));

  act(() => {
    window.dispatchEvent(
      new PopStateEvent("popstate", {
        state: { msimeSettings: true, page: "dictionary", dictionaryDetail: "other" },
      }),
    );
  });

  expect(result.current.value).toBeNull();
});

test("closes locally when there is no matching history entry", () => {
  const { result } = renderHook(useDictionaryDetail);
  act(() => result.current.open("work"));
  window.history.replaceState({ page: "dictionary" }, "");

  act(() => result.current.close());

  expect(result.current.value).toBeNull();
});

test("starts with the subview already represented by the current history entry", () => {
  const { result } = renderHook(() =>
    useMobileSettingsSubview<string>({
      mobile: true,
      stateKey: "dictionaryDetail",
      initialValue: "work",
      decode: (state, currentValue) =>
        typeof state?.dictionaryDetail === "string" &&
        (currentValue === null || state.dictionaryDetail === currentValue)
          ? state.dictionaryDetail
          : null,
      encode: (value) => ({ page: "dictionary", dictionaryDetail: value }),
    }),
  );

  expect(result.current.value).toBe("work");
});
