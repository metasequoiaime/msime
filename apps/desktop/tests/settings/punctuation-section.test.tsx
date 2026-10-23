// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PunctuationSection } from "@msime/ui";
import { PhonePunctuationSection } from "../../../../packages/ui/src/settings/punctuation-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("punctuation controls use existing defaults and send focused preference patches", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );

  expect((screen.getByRole("combobox", { name: /^标点/ }) as HTMLSelectElement).value).toBe(
    "chinese",
  );
  expect(
    (screen.getByRole("switch", { name: /^成对标点自动补全/ }) as HTMLInputElement).checked,
  ).toBe(true);
  // 智能标点默认开，总开关和四个细分开关一起收在「更多选项」里；缺键时重复转中文和两种直出跟着开，空格转换关。
  const smart = screen.getByRole("switch", { name: /^智能标点/ }) as HTMLInputElement;
  expect(smart.checked).toBe(true);
  expect(smart.closest("details")?.textContent).toContain("更多选项");
  expect(screen.queryByRole("switch", { name: "全角输入" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "中文标点" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: /^固定标点/ })).toBeNull();
  for (const [name, checked] of [
    [/^重复标点转中文/, true],
    [/^中文标点后按空格转换/, false],
    [/^数字后直出/, true],
    [/^字母后直出/, true],
  ] as const) {
    const row = screen.getByRole("switch", { name }) as HTMLInputElement;
    expect(row.checked).toBe(checked);
    expect(row.disabled).toBe(false);
    expect(row.closest("details")).toBe(smart.closest("details"));
  }

  // 用户切换总开关时，数字后直出和字母后直出跟着写成同一个值。
  fireEvent.click(smart);
  expect(onChange).toHaveBeenLastCalledWith({
    smart_punctuation: false,
    smart_punctuation_direct_digit: false,
    smart_punctuation_direct_letter: false,
  });
});

test("smart punctuation refinements grey out while the master switch is off", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true, smart_punctuation: false }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  // 智能标点关着时，它的四个细分开关变灰，值不动。
  for (const name of [/^重复标点转中文/, /^中文标点后按空格转换/, /^数字后直出/, /^字母后直出/]) {
    const row = screen.getByRole("switch", { name }) as HTMLInputElement;
    expect(row.disabled).toBe(true);
    expect(row.closest("details")?.textContent).toContain("更多选项");
  }
  fireEvent.click(screen.getByRole("switch", { name: /^智能标点/ }));
  expect(onChange).toHaveBeenLastCalledWith({
    smart_punctuation: true,
    smart_punctuation_direct_digit: true,
    smart_punctuation_direct_letter: true,
  });
});

test("smart punctuation refinements unlock with the master switch and keep their own patches", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        smart_punctuation: true,
        smart_punctuation_direct_digit: true,
        smart_punctuation_direct_letter: false,
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  // 打开页面不改写已存的细分值，单独关掉的字母后直出仍显示为关。
  expect(onChange).not.toHaveBeenCalled();
  const letter = screen.getByRole("switch", { name: /^字母后直出/ }) as HTMLInputElement;
  expect(letter.disabled).toBe(false);
  expect(letter.checked).toBe(false);
  expect((screen.getByRole("switch", { name: /^数字后直出/ }) as HTMLInputElement).checked).toBe(
    true,
  );
  fireEvent.click(letter);
  expect(onChange).toHaveBeenLastCalledWith({ smart_punctuation_direct_letter: true });
  fireEvent.click(screen.getByRole("switch", { name: /^智能标点/ }));
  expect(onChange).toHaveBeenLastCalledWith({
    smart_punctuation: false,
    smart_punctuation_direct_digit: false,
    smart_punctuation_direct_letter: false,
  });
});

test("smart punctuation says it does nothing while both direct halves are off, without rewriting them", () => {
  const onChange = vi.fn();
  const section = (direct_digit: boolean) => (
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        smart_punctuation: true,
        smart_punctuation_direct_digit: direct_digit,
        smart_punctuation_direct_letter: false,
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />
  );
  // 只写过总开关的旧文档：总开关开、两半都关，说明里的承诺不成立，按已存的值提示一句。
  const { rerender } = render(section(false));
  expect(screen.getByText(/这一条暂不生效/)).toBeTruthy();
  expect(onChange).not.toHaveBeenCalled();
  rerender(section(true));
  expect(screen.queryByText(/这一条暂不生效/)).toBeNull();
});

test("标点 reads its value from chinese_punctuation and punctuation_lock and writes only what each choice needs", () => {
  const onChange = vi.fn();
  const mode = () => screen.getByRole("combobox", { name: /^标点/ }) as HTMLSelectElement;
  const { rerender } = render(
    <PunctuationSection
      preferences={{ chinese_punctuation: false }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(Array.from(mode().options).map((option) => option.value)).toEqual([
    "chinese",
    "english",
    "always_chinese",
    "always_english",
  ]);
  expect(mode().value).toBe("english");
  // 锁定优先：锁定时宿主不看 chinese_punctuation，矛盾的旧组合按锁定显示，打开页面不改写。
  for (const [preferences, value] of [
    [{ chinese_punctuation: false, punctuation_lock: "chinese" }, "always_chinese"],
    [{ chinese_punctuation: true, punctuation_lock: "english" }, "always_english"],
    [{ chinese_punctuation: true, punctuation_lock: "follow" }, "chinese"],
  ] as const) {
    rerender(
      <PunctuationSection
        preferences={preferences}
        showCharacterWidth={false}
        onChange={onChange}
      />,
    );
    expect(mode().value).toBe(value);
  }
  expect(onChange).not.toHaveBeenCalled();

  for (const [value, patch] of [
    ["chinese", { chinese_punctuation: true, punctuation_lock: "follow" }],
    ["english", { chinese_punctuation: false, punctuation_lock: "follow" }],
    // 「始终」只写不上云的锁定，起始标点会同步到其他设备，不顺带改写它。
    ["always_chinese", { punctuation_lock: "chinese" }],
    ["always_english", { punctuation_lock: "english" }],
  ] as const) {
    fireEvent.change(mode(), { target: { value } });
    expect(onChange).toHaveBeenLastCalledWith(patch);
  }
});

test("fullwidth control appears only when the host supports it", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true, character_width: "fullwidth" }}
      showCharacterWidth
      onChange={onChange}
    />,
  );

  const fullwidth = screen.getByRole("switch", { name: "全角输入" }) as HTMLInputElement;
  expect(fullwidth.checked).toBe(true);
  fireEvent.click(fullwidth);
  expect(onChange).toHaveBeenLastCalledWith({ character_width: "halfwidth" });
});

test("caps lock punctuation switch appears only on a host that reports caps lock and starts off", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(screen.queryByRole("switch", { name: /^大写锁定时使用英文标点/ })).toBeNull();

  rerender(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      showCapsLockPunctuation
      onChange={onChange}
    />,
  );
  const capsLock = screen.getByRole("switch", {
    name: /^大写锁定时使用英文标点/,
  }) as HTMLInputElement;
  expect(capsLock.checked).toBe(false);
  // 它不依赖智能标点，所以收在「更多选项」里但不变灰。
  expect(capsLock.disabled).toBe(false);
  expect(capsLock.closest("details")?.textContent).toContain("更多选项");
  fireEvent.click(capsLock);
  expect(onChange).toHaveBeenLastCalledWith({ caps_lock_ascii_punctuation: true });
});

test("quote and bracket styles follow 标点, show their defaults and send their stored values", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  const quotes = () => screen.getByRole("combobox", { name: /^引号样式/ }) as HTMLSelectElement;
  const brackets = () =>
    screen.getByRole("combobox", { name: /^方头括号样式/ }) as HTMLSelectElement;
  // 文档里没有这两个键时显示缺省值。
  expect(quotes().value).toBe("curly");
  expect(brackets().value).toBe("solid");
  expect(Array.from(quotes().options).map((option) => option.value)).toEqual([
    "curly",
    "corner",
    "corner_when_traditional",
  ]);
  // 引号样式紧跟在「标点」选择之后、排在成对标点前面；方头括号样式收进「更多选项」，排在顿号自定义与智能标点前面。
  const controls = Array.from(document.querySelectorAll("input, select"));
  const mode = controls.indexOf(screen.getByRole("combobox", { name: /^标点/ }));
  expect(controls.indexOf(quotes())).toBe(mode + 1);
  expect(controls.indexOf(screen.getByRole("switch", { name: /^成对标点自动补全/ }))).toBe(
    mode + 2,
  );
  expect(controls.indexOf(screen.getByRole("switch", { name: /^智能标点/ }))).toBe(
    controls.indexOf(brackets()) + 3,
  );
  expect(quotes().closest("details")).toBeNull();
  expect(brackets().closest("details")?.textContent).toContain("更多选项");

  fireEvent.change(quotes(), { target: { value: "corner" } });
  expect(onChange).toHaveBeenLastCalledWith({ quote_style: "corner" });
  fireEvent.change(brackets(), { target: { value: "hollow" } });
  expect(onChange).toHaveBeenLastCalledWith({ bracket_style: "hollow" });

  rerender(
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        quote_style: "corner_when_traditional",
        bracket_style: "hollow",
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(quotes().value).toBe("corner_when_traditional");
  expect(brackets().value).toBe("hollow");
});

test("顿号自定义两个开关至少留一个", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  const backslash = () =>
    screen.getByRole("switch", { name: /^顿号自定义 \\/ }) as HTMLInputElement;
  const slash = () => screen.getByRole("switch", { name: /^顿号自定义 \// }) as HTMLInputElement;
  // 缺省：\ 开、/ 关；\ 是唯一开着的那个，变灰不可关。
  expect(backslash().checked).toBe(true);
  expect(backslash().disabled).toBe(true);
  expect(slash().checked).toBe(false);
  expect(slash().disabled).toBe(false);
  expect(backslash().closest("details")?.textContent).toContain("更多选项");

  fireEvent.click(slash());
  expect(onChange).toHaveBeenLastCalledWith({
    enumeration_comma_backslash: true,
    enumeration_comma_slash: true,
  });

  // 两个都开时都能点；只剩 / 时 / 变灰。
  rerender(
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        enumeration_comma_backslash: true,
        enumeration_comma_slash: true,
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(backslash().disabled).toBe(false);
  fireEvent.click(backslash());
  expect(onChange).toHaveBeenLastCalledWith({
    enumeration_comma_backslash: false,
    enumeration_comma_slash: true,
  });
  rerender(
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        enumeration_comma_backslash: false,
        enumeration_comma_slash: true,
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(slash().checked).toBe(true);
  expect(slash().disabled).toBe(true);

  // 手改成两个都关的文档按「\ 开」显示，与引擎的读出归一一致。
  rerender(
    <PunctuationSection
      preferences={{
        chinese_punctuation: true,
        enumeration_comma_backslash: false,
        enumeration_comma_slash: false,
      }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );
  expect(backslash().checked).toBe(true);
  expect(slash().checked).toBe(false);
  fireEvent.click(slash());
  expect(onChange).toHaveBeenLastCalledWith({
    enumeration_comma_backslash: true,
    enumeration_comma_slash: true,
  });
});

test("the HarmonyOS phone section keeps both styles under 更多选项", () => {
  const onChange = vi.fn();
  render(
    <PhonePunctuationSection preferences={{ chinese_punctuation: true }} onChange={onChange} />,
  );
  const quotes = screen.getByRole("combobox", { name: /^引号样式/ }) as HTMLSelectElement;
  const brackets = screen.getByRole("combobox", { name: /^方头括号样式/ }) as HTMLSelectElement;
  expect(quotes.closest("details")?.textContent).toContain("更多选项");
  expect(brackets.closest("details")).toBe(quotes.closest("details"));
  fireEvent.change(quotes, { target: { value: "corner" } });
  expect(onChange).toHaveBeenLastCalledWith({ quote_style: "corner" });
});

test("the HarmonyOS phone section greys out smart punctuation refinements and writes both halves with the master", () => {
  const onChange = vi.fn();
  render(
    <PhonePunctuationSection
      preferences={{ chinese_punctuation: true, smart_punctuation: false }}
      onChange={onChange}
    />,
  );
  expect(
    (screen.getByRole("switch", { name: /^重复标点转中文/ }) as HTMLInputElement).disabled,
  ).toBe(true);
  // 手机仍按设计稿保留「使用英文标点」开关和「更多选项」里的「固定标点」，不换成桌面的「标点」选择。
  expect(screen.getByRole("switch", { name: "使用英文标点" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "固定标点" })).toBeTruthy();
  // 智能标点默认开，和桌面一样连同细分开关收在「更多选项」里。
  const smart = screen.getByRole("switch", { name: /^智能标点/ }) as HTMLInputElement;
  expect(smart.closest("details")?.textContent).toContain("更多选项");
  expect(smart.closest("details")).toBe(
    screen.getByRole("switch", { name: /^重复标点转中文/ }).closest("details"),
  );
  fireEvent.click(smart);
  expect(onChange).toHaveBeenLastCalledWith({
    smart_punctuation: true,
    smart_punctuation_direct_digit: true,
    smart_punctuation_direct_letter: true,
  });
});
