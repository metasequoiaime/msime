// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { FontFamilyInput } from "../../../../packages/ui/src/candidate/font-family-input";

afterEach(cleanup);

test.each(
  [
    { key: "ArrowDown", keyCode: 40 },
    { key: "ArrowUp", keyCode: 38 },
    { key: "Enter", keyCode: 13 },
    { key: "Escape", keyCode: 27 },
  ].flatMap(({ key, keyCode }) => [
    { key, keyCode, name: "组词期间", event: { isComposing: true } },
    { key, keyCode, name: "组词结束的兼容事件", event: { isComposing: false, keyCode: 229 } },
  ]),
)("字体选择器不截获$name 的 $key", ({ key, keyCode, event }) => {
  const onChange = vi.fn();
  render(
    <FontFamilyInput
      label="合成字体"
      value="示例"
      onChange={onChange}
      fonts={["示例甲", "示例乙"]}
      enabled
      ready
      request={vi.fn()}
    />,
  );
  const input = screen.getByRole("combobox", { name: "合成字体" });
  fireEvent.focus(input);
  fireEvent.keyDown(input, { key: "ArrowDown", keyCode: 40 });
  expect(screen.getByRole("option", { name: "示例甲" }).getAttribute("aria-selected")).toBe("true");

  expect(fireEvent.keyDown(input, { key, ...event })).toBe(true);
  expect(onChange).not.toHaveBeenCalled();
  expect(input.getAttribute("aria-expanded")).toBe("true");
  expect(screen.getByRole("option", { name: "示例甲" }).getAttribute("aria-selected")).toBe("true");

  expect(fireEvent.keyDown(input, { key, isComposing: false, keyCode })).toBe(false);
  if (key === "Enter") {
    expect(onChange).toHaveBeenCalledWith("示例甲");
    expect(input.getAttribute("aria-expanded")).toBe("false");
  } else if (key === "Escape") {
    expect(input.getAttribute("aria-expanded")).toBe("false");
  } else {
    expect(screen.getByRole("option", { name: "示例乙" }).getAttribute("aria-selected")).toBe(
      "true",
    );
  }
});
