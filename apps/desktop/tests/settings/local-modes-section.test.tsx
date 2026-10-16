// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LocalModesSection, type LocalModePreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
};

test("local mode section updates only the selected mode", () => {
  const onChange = vi.fn();
  render(<LocalModesSection preferences={preferences} ios={false} onChange={onChange} />);

  fireEvent.click(screen.getByRole("switch", { name: /^Unicode/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, unicode: false });
});

test("iOS local mode descriptions use the touch keyboard entry path", () => {
  render(<LocalModesSection preferences={preferences} ios onChange={vi.fn()} />);

  expect(screen.getByText(/「更多 → 本地输入」里选「快捷短语」/)).toBeTruthy();
  expect(screen.queryByText(/Shift\+K/)).toBeNull();
});

test("the Shift+letter master switch is shown only where the host asks for it", () => {
  render(<LocalModesSection preferences={preferences} ios={false} onChange={vi.fn()} />);
  expect(screen.queryByRole("switch", { name: /Shift\+字母进入快捷模式/ })).toBeNull();
});

test("turning the Shift+letter master switch off keeps every mode switch as it was", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <LocalModesSection
      preferences={preferences}
      ios={false}
      triggers
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  const master = screen.getByRole("switch", { name: /Shift\+字母进入快捷模式/ });
  // 早于总开关的文档没有这个键，读作开。
  expect((master as HTMLInputElement).checked).toBe(true);
  fireEvent.click(master);
  expect(onChange).toHaveBeenCalledWith({ ...preferences, shift_letter_entry: false });

  rerender(
    <LocalModesSection
      preferences={{ ...preferences, shift_letter_entry: false }}
      ios={false}
      triggers
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  // Shift+字母的模式暂时不起作用，开关保留原值但不能改；/ 不经过 Shift，照常可改。
  const unicode = screen.getByRole("switch", { name: /^Unicode/ });
  expect((unicode as HTMLInputElement).checked).toBe(true);
  expect((unicode as HTMLInputElement).disabled).toBe(true);
  const expression = screen.getByRole("switch", { name: /V 模式/ });
  expect((expression as HTMLInputElement).disabled).toBe(true);
  const command = screen.getByRole("switch", { name: /\/ 模式/ });
  expect((command as HTMLInputElement).disabled).toBe(false);
});

test("拆字反查缺键时桌面读作关、触屏读作开，切换后写回明确的布尔", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <LocalModesSection
      preferences={preferences}
      ios={false}
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  // 桌面宿主（显示总开关的那三个）缺键读作关，和 host-api 在桌面构建里的缺省一致。
  const desktop = screen.getByRole("switch", { name: /^拆字反查/ });
  expect((desktop as HTMLInputElement).checked).toBe(false);
  fireEvent.click(desktop);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, chaizi: true });

  // 总开关关掉时拆字和其他 Shift+字母模式一样置灰，值保留。
  rerender(
    <LocalModesSection
      preferences={{ ...preferences, shift_letter_entry: false, chaizi: true }}
      ios={false}
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  const disabled = screen.getByRole("switch", { name: /^拆字反查/ });
  expect((disabled as HTMLInputElement).checked).toBe(true);
  expect((disabled as HTMLInputElement).disabled).toBe(true);

  // 触屏宿主不显示总开关，缺键读作开；关掉时写回明确的 false。
  rerender(<LocalModesSection preferences={preferences} ios onChange={onChange} />);
  const touch = screen.getByRole("switch", { name: /^拆字反查/ });
  expect((touch as HTMLInputElement).checked).toBe(true);
  expect(screen.getByText(/「更多 → 本地输入」里选「拆字」/)).toBeTruthy();
  fireEvent.click(touch);
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, chaizi: false });
});

test("desktop folds the per-mode switches into 逐个模式 under the master switch", () => {
  render(
    <LocalModesSection
      preferences={preferences}
      ios={false}
      shiftLetterEntrySwitch
      temporaryJapaneseExtra={<span>日文词库下载行</span>}
      onChange={vi.fn()}
    />,
  );
  const master = screen.getByRole("switch", { name: /Shift\+字母进入快捷模式/ });
  const modes = screen.getByRole("region", { name: "逐个模式" });
  const fold = modes.querySelector("details");
  expect(fold?.open).toBe(false);
  expect(fold?.contains(master)).toBe(false);
  expect(fold?.contains(screen.getByRole("switch", { name: /^快捷短语/ }))).toBe(true);
  // 日文词库的下载行紧跟 R 模式那一行，不再单占一组。
  const japanese = screen.getByRole("switch", { name: /^临时日语/ });
  const download = screen.getByText("日文词库下载行");
  expect(fold?.contains(download)).toBe(true);
  expect(
    japanese.compareDocumentPosition(download) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
});

test("touch hosts without the master switch keep the modes listed flat", () => {
  render(<LocalModesSection preferences={preferences} ios onChange={vi.fn()} />);
  expect(screen.queryByText("逐个模式")).toBeNull();
  expect(document.querySelector("details")).toBeNull();
});
