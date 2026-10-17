// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { useCustomEndpointIntent } from "@msime/ui";

afterEach(cleanup);

test("保留手动展开意图，并在编辑类别或服务后重置", () => {
  const hook = renderHook(
    ({ baseCustom, resetKey }) => useCustomEndpointIntent({ baseCustom, resetKey }),
    { initialProps: { baseCustom: false, resetKey: "asr:doubao" } },
  );

  expect(hook.result.current.custom).toBe(false);
  act(() => hook.result.current.setCustom(true));
  expect(hook.result.current.custom).toBe(true);
  hook.rerender({ baseCustom: false, resetKey: "asr:doubao" });
  expect(hook.result.current.custom).toBe(true);
  hook.rerender({ baseCustom: true, resetKey: "asr:doubao" });
  expect(hook.result.current.custom).toBe(true);
  hook.rerender({ baseCustom: false, resetKey: "polish:doubao" });
  expect(hook.result.current.custom).toBe(false);
});
