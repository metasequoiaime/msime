// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type Kind = "pinyin" | "wubi" | "quick" | "english";
type CloudDictionaryKindTabsProps = {
  value: Kind;
  disabled?: boolean;
  onChange: (kind: Kind) => void;
};

test("cloud dictionary kind tabs render all kinds and forward the selection", () => {
  const Tabs = (
    ui as unknown as { CloudDictionaryKindTabs: ComponentType<CloudDictionaryKindTabsProps> }
  ).CloudDictionaryKindTabs;
  expect(Tabs).toBeDefined();

  const onChange = vi.fn();
  render(<Tabs value="pinyin" onChange={onChange} />);

  expect(screen.getByRole("tab", { name: "拼音" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.click(screen.getByRole("tab", { name: "快捷短语" }));
  expect(onChange).toHaveBeenCalledWith("quick");
});
