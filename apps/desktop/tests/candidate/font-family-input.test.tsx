// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { FontFamilyInput } from "../../../../packages/ui/src/candidate/font-family-input";
import { cssValue } from "../support/css-value";

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

function renderInClippedCard() {
  render(
    <div data-settings-shell="">
      <section data-testid="card" style={{ overflow: "hidden" }}>
        <FontFamilyInput
          label="合成字体"
          value=""
          onChange={vi.fn()}
          fonts={["示例甲", "示例乙", "示例丙"]}
          enabled
          ready
          request={vi.fn()}
        />
      </section>
    </div>,
  );
  return screen.getByRole("combobox", { name: "合成字体" });
}

function placeField(input: HTMLElement, top: number) {
  const box = input.parentElement!;
  vi.spyOn(box, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 300, y: top, width: 240, height: 28 }),
  );
}

// #11889：设置卡片 `overflow: hidden`，菜单挂在卡片里会被卡片底边截掉，只露出几项。
test("字体菜单挂到设置外壳上、按视口定位，不被所在卡片裁切", () => {
  const input = renderInClippedCard();
  placeField(input, 100);
  fireEvent.focus(input);
  const menu = screen.getByRole("listbox", { name: "合成字体可用字体" }).parentElement!;
  expect(screen.getByTestId("card").contains(menu)).toBe(false);
  expect(menu.parentElement?.hasAttribute("data-settings-shell")).toBe(true);
  expect(menu.style.position).toBe("fixed");
  expect(menu.style.left).toBe("300px");
  expect(menu.style.top).toBe("132px");
  expect(menu.style.width).toBe("240px");
  expect(menu.style.maxHeight).toBe("240px");
  expect(menu.style.translate).toBe("");
});

test("字段下方放不下时菜单往上弹", () => {
  const input = renderInClippedCard();
  placeField(input, window.innerHeight - 60);
  fireEvent.focus(input);
  const menu = screen.getByRole("listbox", { name: "合成字体可用字体" }).parentElement!;
  expect(menu.style.top).toBe(`${window.innerHeight - 64}px`);
  expect(menu.style.translate).toBe(cssValue("translate", "0 -100%"));
  expect(menu.style.maxHeight).toBe(`${Math.min(240, window.innerHeight - 72)}px`);
});

test("页面滚动时菜单收起，菜单自身滚动不收起", () => {
  const input = renderInClippedCard();
  fireEvent.focus(input);
  const list = screen.getByRole("listbox", { name: "合成字体可用字体" });
  fireEvent.scroll(list.parentElement!);
  expect(input.getAttribute("aria-expanded")).toBe("true");
  fireEvent.scroll(screen.getByTestId("card"));
  expect(input.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("listbox")).toBeNull();
});
