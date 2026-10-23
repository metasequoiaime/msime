// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  InputLanguageOptionsSection,
  MixedInputMoreRows,
  MixedInputSection,
  type MixedInputPreferences,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: MixedInputPreferences = {
  english: true,
  minimum_prefix: 5,
  emoji: false,
  kaomoji: false,
};

test("mixed input preserves the threshold while toggling independent switches", () => {
  const onChange = vi.fn();
  render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  const threshold = screen.getByRole("combobox", { name: "触发字符数" }) as HTMLSelectElement;
  fireEvent.change(threshold, { target: { value: "8" } });
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, minimum_prefix: 8 });

  fireEvent.click(screen.getByRole("switch", { name: /^emoji 混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, emoji: true });
  // 符号、颜文字和日文混输不在这几行里，由页面收进「更多选项」。
  expect(screen.queryByRole("switch", { name: /^颜文字混输/ })).toBeNull();
  expect(screen.queryByRole("switch", { name: /^符号混输/ })).toBeNull();
});

test("a host that fixes the threshold gets no 触发字符数 row", () => {
  render(
    <MixedInputSection preferences={preferences} showMinimumPrefix={false} onChange={vi.fn()} />,
  );
  expect(screen.queryByRole("combobox", { name: "触发字符数" })).toBeNull();
  expect(screen.getByRole("switch", { name: /^中英混输/ })).toBeTruthy();
  expect(screen.getByRole("switch", { name: /^emoji 混输/ })).toBeTruthy();
});

// 多语言区块也能画混输几行，固定触发字符数的宿主在那里同样不能把这一行带回来。
test("the language options section carries the 触发字符数 gate through", () => {
  const view = render(
    <InputLanguageOptionsSection
      includeMixed
      mixedInput={preferences}
      showMixedInputMinimumPrefix={false}
      onMixedInputChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("combobox", { name: "触发字符数" })).toBeNull();
  expect(screen.getByRole("switch", { name: /^中英混输/ })).toBeTruthy();
  view.rerender(
    <InputLanguageOptionsSection
      includeMixed
      mixedInput={preferences}
      onMixedInputChange={vi.fn()}
    />,
  );
  expect(screen.getByRole("combobox", { name: "触发字符数" })).toBeTruthy();
});

test("the folded rows toggle kaomoji on their own", () => {
  const onChange = vi.fn();
  render(<MixedInputMoreRows preferences={preferences} onChange={onChange} />);
  fireEvent.click(screen.getByRole("switch", { name: /^颜文字混输/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, kaomoji: true });
});

test("disabling English mixed input disables its threshold without changing its value", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputSection preferences={preferences} onChange={onChange} />);

  fireEvent.click(screen.getByRole("switch", { name: /^中英混输/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, english: false });
  view.rerender(
    <MixedInputSection preferences={{ ...preferences, english: false }} onChange={onChange} />,
  );
  const threshold = screen.getByRole("combobox", { name: "触发字符数" }) as HTMLSelectElement;
  expect(threshold.disabled).toBe(true);
  expect(threshold.value).toBe("5");
});

test("Japanese mixed input is off when the document predates it and toggles on its own", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputMoreRows preferences={preferences} onChange={onChange} />);

  const japanese = screen.getByRole("switch", { name: /^日文混输（实验）/ }) as HTMLInputElement;
  expect(japanese.checked).toBe(false);
  fireEvent.click(japanese);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, japanese: true });

  view.rerender(
    <MixedInputMoreRows preferences={{ ...preferences, japanese: true }} onChange={onChange} />,
  );
  expect(
    (screen.getByRole("switch", { name: /^日文混输（实验）/ }) as HTMLInputElement).checked,
  ).toBe(true);
  fireEvent.click(screen.getByRole("switch", { name: /^日文混输（实验）/ }));
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, japanese: false });
});

test("symbol mixed input is on when the document predates it and writes false only when turned off", () => {
  const onChange = vi.fn();
  const view = render(<MixedInputMoreRows preferences={preferences} onChange={onChange} />);

  const symbols = screen.getByRole("switch", { name: /^符号混输/ }) as HTMLInputElement;
  expect(symbols.checked).toBe(true);
  fireEvent.click(symbols);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, symbols: false });

  view.rerender(
    <MixedInputMoreRows preferences={{ ...preferences, symbols: false }} onChange={onChange} />,
  );
  const off = screen.getByRole("switch", { name: /^符号混输/ }) as HTMLInputElement;
  expect(off.checked).toBe(false);
  fireEvent.click(off);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, symbols: true });
});

test("an edition without temporary Japanese does not list Japanese mixed input", () => {
  render(
    <MixedInputMoreRows
      preferences={{ ...preferences, japanese: true }}
      temporaryJapanese={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.queryByRole("switch", { name: /^日文混输（实验）/ })).toBeNull();
  expect(screen.getByRole("switch", { name: /^颜文字混输/ })).toBeTruthy();
});

test("关闭混输总开关也禁用快捷短语及调频，并保留两项选择", () => {
  const onChange = vi.fn();
  const stored = Object.freeze({
    ...preferences,
    enabled: false,
    quick_phrase: true,
    quick_phrase_frequency: true,
  });
  const view = render(
    <MixedInputMoreRows preferences={stored} quickPhraseCandidates onChange={onChange} />,
  );
  for (const name of [/^快捷短语混输/, /^快捷短语参与调频/]) {
    const control = screen.getByRole("switch", { name }) as HTMLInputElement;
    expect(control.checked).toBe(true);
    expect(control.disabled).toBe(true);
    control.click();
  }
  expect(onChange).not.toHaveBeenCalled();
  view.rerender(
    <MixedInputMoreRows
      preferences={{ ...stored, enabled: true }}
      quickPhraseCandidates
      onChange={onChange}
    />,
  );
  for (const name of [/^快捷短语混输/, /^快捷短语参与调频/]) {
    const control = screen.getByRole("switch", { name }) as HTMLInputElement;
    expect(control.checked).toBe(true);
    expect(control.disabled).toBe(false);
  }
});
