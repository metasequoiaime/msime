// @vitest-environment jsdom
import { expect, test } from "vitest";
import { renderHook } from "@testing-library/react";
import { useLatestRef } from "../../../../packages/ui/src/core/use-latest-ref";

test("keeps one ref while exposing the latest value", () => {
  const { result, rerender } = renderHook(({ value }) => useLatestRef(value), {
    initialProps: { value: "first" },
  });
  const ref = result.current;

  expect(ref.current).toBe("first");
  rerender({ value: "second" });

  expect(result.current).toBe(ref);
  expect(ref.current).toBe("second");
});
