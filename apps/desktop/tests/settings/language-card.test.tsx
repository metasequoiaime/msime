// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Preferences, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { LanguageCard } from "../../../../packages/ui/src/settings/language-card";
import {
  defaultTouchKeyboardSchemes,
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type TouchKeyboardScheme,
} from "../../../../packages/ui/src/settings/touch-keyboard-scheme-helpers";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

// 没有藏文、粤语、注音和笔画词库的 HarmonyOS 宿主所提供的内容。
const available: TouchKeyboardScheme[] = [
  "quanpin",
  "nine_key",
  "xiaohe",
  "ziranma",
  "microsoft",
  "shoudao",
  "wubi",
  "japanese_nine_key",
  "japanese",
  "handwriting",
  "korean",
  "vietnamese",
];

/** 按设置表单的接法连接的卡片：选中即启用，切换走共享辅助函数，保证至少开着一个方案。 */
function Harness({
  initial,
  onPreferences,
  offered = available,
}: {
  initial: Preferences;
  onPreferences: (preferences: Preferences) => void;
  offered?: TouchKeyboardScheme[];
}) {
  const [preferences, setPreferences] = useState(initial);
  onPreferences(preferences);
  return (
    <LanguageCard
      available={offered}
      enabled={preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes}
      selected={inferredTouchKeyboardScheme(preferences)}
      wubiProfile={preferences.wubi_profile}
      onSelect={(scheme) =>
        setPreferences((current) => selectHomeTouchKeyboardScheme(current, scheme))
      }
      onToggle={(scheme, enabled) =>
        setPreferences(
          (current) =>
            updateTouchKeyboardSchemeEnabled(
              current,
              scheme,
              enabled,
              inferredTouchKeyboardScheme(current),
            ) ?? current,
        )
      }
      onWubiProfileChange={(wubi_profile) =>
        setPreferences((current) => ({ ...current, wubi_profile }))
      }
    />
  );
}

function renderCard(
  enabled: TouchKeyboardScheme[],
  selected?: TouchKeyboardScheme,
  offered?: TouchKeyboardScheme[],
) {
  let latest = base;
  const initial = selectHomeTouchKeyboardScheme(
    { ...base, touch_keyboard_schemes: { enabled } },
    selected ?? enabled[0],
  );
  render(<Harness initial={initial} onPreferences={(next) => (latest = next)} offered={offered} />);
  return () => latest;
}

function languageRow(name: string) {
  return within(screen.getByRole("group", { name: "输入方案" })).getByRole("button", {
    name: new RegExp(`^${name}`),
  });
}

function sheet() {
  return screen.getByRole("dialog");
}

test("groups the enabled schemes by language and offers the rest under 添加语言", () => {
  renderCard(["quanpin", "nine_key", "japanese"]);

  const card = screen.getByRole("group", { name: "输入方案" });
  expect(languageRow("普通话").textContent).toContain("全拼");
  expect(languageRow("日语").textContent).toContain("26 键");
  // 韩语和越南语有提供但处于关闭状态，所以它们在添加语言下等着；宿主完全不提供粤语和藏文。
  expect(within(card).queryByRole("button", { name: /^韩语/ })).toBeNull();
  expect(within(card).queryByRole("button", { name: /^粤语/ })).toBeNull();
  expect(within(card).queryByRole("button", { name: "添加韩语" })).toBeNull();

  fireEvent.click(within(card).getByRole("button", { name: "添加语言" }));
  expect(within(card).getByRole("button", { name: "添加韩语" })).toBeTruthy();
  expect(within(card).getByRole("button", { name: "添加越南语" })).toBeTruthy();
  expect(within(card).queryByRole("button", { name: "添加粤语" })).toBeNull();
  expect(within(card).queryByRole("button", { name: "添加藏语" })).toBeNull();
  expect(within(card).getByRole("button", { name: "完成" })).toBeTruthy();
});

test("the 普通话 sheet lists every offered scheme, with 双拼 and 五笔 as submenus and no remove", () => {
  const preferences = renderCard(["quanpin"]);

  fireEvent.click(languageRow("普通话"));
  const options = within(sheet())
    .getAllByRole("button")
    .filter((button) => button.hasAttribute("data-sheet-option"))
    .map((button) => button.textContent?.replace(/\s*›$/, ""));
  expect(options).toEqual(["全拼", "全拼 9 键", "双拼（小鹤）", "五笔（五笔 86）", "手写"]);
  expect(within(sheet()).getByRole("button", { name: "全拼" }).getAttribute("aria-current")).toBe(
    "true",
  );

  // 选择一个原本关闭的双拼方案会开启它并设为当前方案。
  fireEvent.click(within(sheet()).getByRole("button", { name: "双拼（小鹤）" }));
  expect(
    within(sheet())
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["小鹤", "自然码", "微软", "首道"]);
  fireEvent.click(within(sheet()).getByRole("button", { name: "自然码" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(preferences()).toMatchObject({
    scheme: "shuangpin",
    shuangpin_profile: "ziranma",
    touch_keyboard_schemes: { enabled: ["quanpin", "ziranma"], selected: "ziranma" },
  });
  expect(languageRow("普通话").textContent).toContain("双拼 · 自然码");
});

test("the 普通话 sheet offers 全拼 14 键 between 26 键 and 9 键 when the host has it", () => {
  const preferences = renderCard(["quanpin"], undefined, [...available, "fourteen_key"]);

  fireEvent.click(languageRow("普通话"));
  const options = within(sheet())
    .getAllByRole("button")
    .filter((button) => button.hasAttribute("data-sheet-option"))
    .map((button) => button.textContent?.replace(/\s*›$/, ""));
  expect(options.slice(0, 3)).toEqual(["全拼", "全拼 14 键", "全拼 9 键"]);
  fireEvent.click(within(sheet()).getByRole("button", { name: "全拼 14 键" }));
  expect(preferences()).toMatchObject({
    scheme: "quanpin",
    touch_keyboard_layout: "fourteen_key",
    touch_keyboard_schemes: { enabled: ["quanpin", "fourteen_key"], selected: "fourteen_key" },
  });
  expect(languageRow("普通话").textContent).toContain("全拼 14 键");
});

test("a 五笔 profile picked in its submenu sets the profile and switches to 五笔", () => {
  const preferences = renderCard(["quanpin"]);

  fireEvent.click(languageRow("普通话"));
  fireEvent.click(within(sheet()).getByRole("button", { name: "五笔（五笔 86）" }));
  fireEvent.click(within(sheet()).getByRole("button", { name: "五笔 98" }));

  expect(preferences()).toMatchObject({
    scheme: "wubi",
    wubi_profile: "wubi98",
    touch_keyboard_schemes: { enabled: ["quanpin", "wubi"], selected: "wubi" },
  });
  expect(languageRow("普通话").textContent).toContain("五笔 98");
});

test("removing a language in use turns off all its schemes and falls back to another language", () => {
  const preferences = renderCard(["quanpin", "japanese_nine_key", "japanese"], "japanese");

  fireEvent.click(languageRow("日语"));
  expect(
    within(sheet())
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["26 键", "9 键", "移除日语"]);
  fireEvent.click(within(sheet()).getByRole("button", { name: "移除日语" }));

  expect(preferences()).toMatchObject({
    scheme: "quanpin",
    touch_keyboard_schemes: { enabled: ["quanpin"], selected: "quanpin" },
  });
  const card = screen.getByRole("group", { name: "输入方案" });
  expect(within(card).queryByRole("button", { name: /^日语/ })).toBeNull();
  fireEvent.click(within(card).getByRole("button", { name: "添加语言" }));
  expect(within(card).getByRole("button", { name: "添加日语" })).toBeTruthy();
});

test("a language holding the only enabled schemes cannot be removed", () => {
  const preferences = renderCard(["korean"]);

  // 普通话在没有方案开启时仍保留它的行；它的面板从不提供移除。
  expect(languageRow("普通话")).toBeTruthy();
  fireEvent.click(languageRow("韩语"));
  const remove = within(sheet()).getByRole("button", { name: "移除韩语" }) as HTMLButtonElement;
  expect(remove.disabled).toBe(true);
  fireEvent.click(remove);
  expect(preferences().touch_keyboard_schemes?.enabled).toEqual(["korean"]);
});

test("adding a language turns on its first scheme without switching to it", () => {
  const preferences = renderCard(["quanpin"]);

  const card = screen.getByRole("group", { name: "输入方案" });
  fireEvent.click(within(card).getByRole("button", { name: "添加语言" }));
  fireEvent.click(within(card).getByRole("button", { name: "添加日语" }));

  expect(preferences()).toMatchObject({
    scheme: "quanpin",
    touch_keyboard_schemes: { enabled: ["quanpin", "japanese"], selected: "quanpin" },
  });
  expect(languageRow("日语").textContent).toContain("26 键");
  expect(within(card).queryByRole("button", { name: "添加日语" })).toBeNull();
  expect(within(card).getByRole("button", { name: "完成" })).toBeTruthy();
});

// ---- 卡片所在的输入页 ----

const snapshot: Snapshot = { format_version: 1, revision: 4, preferences: base };

function renderInputPage(platform: string, host: Record<string, unknown> = {}) {
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save,
        host: testHost({ platform, ...host }),
        fuzzyPinyin: true,
        touchKeyboardSchemes: true,
        candidateEnglishGloss: true,
      }}
    />,
  );
  return save;
}

function inputPage() {
  return screen.getByRole("group", { name: "输入" });
}

/** 页面自己的分组，不包括折叠在其他分组里的。 */
function topGroups(page: HTMLElement) {
  return [...page.querySelectorAll("section")].filter(
    (section) => !section.parentElement?.closest("section"),
  );
}

function groupTitles(page: HTMLElement) {
  return topGroups(page).map(
    (section) => section.querySelector(":scope > [data-group-title]")?.textContent,
  );
}

test("the HarmonyOS phone 输入 page shows the design's groups and folds the rest into 更多", async () => {
  renderInputPage("harmony");
  await settingsFormReady();
  const page = inputPage();

  expect(groupTitles(page)).toEqual(["语言与方案", "中文", "辅助码", "翻译", "更多"]);
  expect(within(page).getByRole("group", { name: "输入方案" })).toBeTruthy();
  expect(within(page).queryByRole("switch", { name: /^显示输入方案/ })).toBeNull();
  // 辅助码分组列出当前方案族的码表，另一族的留在折叠区里。
  expect(within(page).getByRole("combobox", { name: "全拼辅助码方案" })).toBeTruthy();
  expect(within(page).getByRole("combobox", { name: "双拼辅助码方案" })).toBeTruthy();
  expect(within(page).getByText("在候选词下方显示英语释义")).toBeTruthy();
  expect(within(page).getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
  // 中英文设置和其余设置仍可在更多下找到；整句联想和翻页键在那里属于其他页面。
  const more = topGroups(page).at(-1) as HTMLElement;
  expect(within(more).getByRole("switch", { name: "以词定字" })).toBeTruthy();
  expect(within(more).getByText("拼音方案调频")).toBeTruthy();
  expect(within(page).queryByRole("switch", { name: "本地整句联想" })).toBeNull();
  expect(within(page).queryByRole("switch", { name: "繁体输出" })).toBeNull();
});

test("拼音纠错 sets both quanpin corrections and 中文字符集 sets traditional output", async () => {
  const save = renderInputPage("harmony");
  await settingsFormReady();
  const page = inputPage();

  const autocorrect = within(page).getByRole("switch", { name: "拼音纠错" }) as HTMLInputElement;
  expect(autocorrect.checked).toBe(true);
  fireEvent.click(autocorrect);
  fireEvent.change(within(page).getByRole("combobox", { name: "中文字符集" }), {
    target: { value: "traditional" },
  });
  saveSettingsNow();

  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      4,
      expect.objectContaining({
        quanpin: { autocorrect_transposition: false, autocorrect_neighbor: false },
        traditional_chinese_output: true,
      }),
    ),
  );
});

test("the desktop 输入 page keeps its own groups", async () => {
  renderInputPage("windows");
  await settingsFormReady();
  const page = inputPage();

  expect(groupTitles(page)).toContain("方案");
  expect(groupTitles(page)).toContain("选词与翻页");
  expect(groupTitles(page)).not.toContain("语言与方案");
  expect(groupTitles(page)).not.toContain("更多");
  expect(within(page).getByRole("switch", { name: "繁体输出" })).toBeTruthy();
});
