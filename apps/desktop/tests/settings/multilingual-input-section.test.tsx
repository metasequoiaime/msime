// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MultilingualInputSection } from "../../../../packages/ui/src/settings/multilingual-input-section";

afterEach(cleanup);

test("默认关闭；开启时保留默认语言集合", () => {
  const onChange = vi.fn();
  render(<MultilingualInputSection temporaryJapanese onChange={onChange} />);
  const enabled = screen.getByRole("switch", { name: "连续多语言输入（实验）" });
  expect((enabled as HTMLInputElement).checked).toBe(false);
  expect(screen.getByRole("switch", { name: "连续输入：中文" }).hasAttribute("disabled")).toBe(
    true,
  );
  fireEvent.click(enabled);
  expect(onChange).toHaveBeenCalledWith({
    enabled: true,
    languages: ["chinese", "english", "japanese"],
  });
});

test("显式空列表保持全不选，选择语言不自动补齐其它语言", () => {
  const onChange = vi.fn();
  render(
    <MultilingualInputSection
      preferences={{ enabled: true, languages: [] }}
      temporaryJapanese
      onChange={onChange}
    />,
  );
  expect((screen.getByRole("switch", { name: "连续输入：中文" }) as HTMLInputElement).checked).toBe(
    false,
  );
  fireEvent.click(screen.getByRole("switch", { name: "连续输入：英文" }));
  expect(onChange).toHaveBeenCalledWith({ enabled: true, languages: ["english"] });
});

test("关闭只改总开关；不支持日文的版本保留存储选择", () => {
  const onChange = vi.fn();
  render(
    <MultilingualInputSection
      preferences={{ enabled: true, languages: ["english", "japanese"] }}
      temporaryJapanese={false}
      onChange={onChange}
    />,
  );
  expect(screen.queryByRole("switch", { name: "连续输入：日文" })).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "连续多语言输入（实验）" }));
  expect(onChange).toHaveBeenCalledWith({ enabled: false, languages: ["english", "japanese"] });
});
