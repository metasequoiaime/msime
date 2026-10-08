// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type EditionInfo, type Preferences, type Snapshot } from "@msime/ui";
import { PhonePunctuationSection } from "../../../../packages/ui/src/settings/punctuation-section";
import {
  SentenceAssociationLevelRow,
  sentenceAssociationLevel,
  sentenceAssociationWithLevel,
} from "../../../../packages/ui/src/settings/sentence-association-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base: Snapshot = {
  format_version: 1,
  revision: 4,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    candidate_translations: true,
  },
};

const fullEdition: EditionInfo = {
  id: "full",
  input_schemes: ["quanpin", "shuangpin", "wubi"],
  default_scheme: "quanpin",
  temporary_japanese: true,
  neural_keyboard: true,
  offline_glosses: true,
  handwriting: true,
  wubi_mixed_pinyin_default: false,
};

async function mountPhone(
  preferences: Partial<Preferences> = {},
  host: Record<string, unknown> = {},
  platform = "harmony",
) {
  const snapshot: Snapshot = { ...base, preferences: { ...base.preferences, ...preferences } };
  // 像宿主那样把保存的文档回传，让第二次保存从第一次的结果开始。
  const save = vi.fn(async (revision: number, saved: Preferences) => ({
    ...snapshot,
    revision: revision + 1,
    preferences: saved,
  }));
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => snapshot,
        save,
        host: testHost({ platform, english_suggestions: true, ...host }),
      }}
    />,
  );
  await settingsFormReady();
  return save;
}

async function saveAndRead(save: ReturnType<typeof vi.fn>, call: number) {
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(call + 1));
  return save.mock.calls[call][1] as Preferences;
}

const group = (name: string) => screen.getByRole("region", { name });

describe("整句联想 levels", () => {
  test("the stored switches read as the Android levels", () => {
    expect(sentenceAssociationLevel(undefined)).toBe(1);
    expect(sentenceAssociationLevel({})).toBe(1);
    expect(sentenceAssociationLevel({ word_lattice: false })).toBe(0);
    // 关闭的 lattice 优先于仍开着的 neural 开关。
    expect(sentenceAssociationLevel({ word_lattice: false, neural_keyboard: true })).toBe(0);
    expect(sentenceAssociationLevel({ neural_keyboard: true })).toBe(2);
    expect(sentenceAssociationLevel({ word_lattice: true, neural_keyboard: false })).toBe(1);
  });

  test("a level is written as both switches and keeps the other fields", () => {
    const stored = { neural_desktop: true, show_next_on_duplicate: false };
    expect(sentenceAssociationWithLevel(stored, 0)).toEqual({
      ...stored,
      word_lattice: false,
      neural_keyboard: false,
    });
    expect(sentenceAssociationWithLevel(stored, 1)).toEqual({
      ...stored,
      word_lattice: true,
      neural_keyboard: false,
    });
    expect(sentenceAssociationWithLevel(undefined, 2)).toEqual({
      word_lattice: true,
      neural_keyboard: true,
    });
  });

  test("the row offers 增强 only with the keyboard neural model", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <SentenceAssociationLevelRow value={{ neural_keyboard: true }} onChange={onChange} />,
    );
    const select = () => screen.getByRole("combobox", { name: /整句联想/ }) as HTMLSelectElement;
    expect(Array.from(select().options).map((option) => option.textContent)).toEqual([
      "关闭",
      "标准",
      "增强",
    ]);
    expect(select().value).toBe("2");
    fireEvent.change(select(), { target: { value: "0" } });
    expect(onChange).toHaveBeenLastCalledWith({ word_lattice: false, neural_keyboard: false });

    rerender(
      <SentenceAssociationLevelRow
        value={{ neural_keyboard: true }}
        neuralKeyboard={false}
        onChange={onChange}
      />,
    );
    expect(Array.from(select().options).map((option) => option.textContent)).toEqual([
      "关闭",
      "标准",
    ]);
    // 在不带模型的版本里，存储的「增强」按「标准」运行，所以行上显示的是「标准」。
    expect(select().value).toBe("1");
  });
});

test("使用英文标点 is the inverse of chinese_punctuation", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <PhonePunctuationSection preferences={{ chinese_punctuation: true }} onChange={onChange} />,
  );
  const english = () => screen.getByRole("switch", { name: "使用英文标点" }) as HTMLInputElement;
  expect(english().checked).toBe(false);
  fireEvent.click(english());
  expect(onChange).toHaveBeenLastCalledWith({ chinese_punctuation: false });

  rerender(
    <PhonePunctuationSection preferences={{ chinese_punctuation: false }} onChange={onChange} />,
  );
  expect(english().checked).toBe(true);
  fireEvent.click(english());
  expect(onChange).toHaveBeenLastCalledWith({ chinese_punctuation: true });

  const paired = screen.getByRole("switch", { name: "自动补全成对标点" }) as HTMLInputElement;
  expect(paired.checked).toBe(true);
  fireEvent.click(paired);
  expect(onChange).toHaveBeenLastCalledWith({ paired_punctuation: false });
  // 「固定标点」和智能标点的细化选项仍可在「更多选项」下找到。
  expect(screen.getByRole("combobox", { name: "固定标点" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: /^重复标点转中文/ })).toBeTruthy();
});

describe("the HarmonyOS phone 表达 page", () => {
  test("shows the design's 标点 and 智能 groups and drops the 高情商回复 card", async () => {
    await mountPhone();
    expect(screen.getByRole("group", { name: "表达" })).toBeTruthy();
    const punctuation = group("标点");
    expect(
      (within(punctuation).getByRole("switch", { name: "使用英文标点" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
    expect(within(punctuation).queryByRole("switch", { name: "中文标点" })).toBeNull();
    const smart = group("智能");
    expect(within(smart).getByRole("button", { name: /整句联想.*标准/ })).toBeTruthy();
    expect(
      (within(smart).getByRole("switch", { name: "英文联想" }) as HTMLInputElement).checked,
    ).toBe(true);
    expect(screen.queryByRole("region", { name: "高情商回复" })).toBeNull();
    expect(screen.queryByRole("region", { name: "候选词翻译" })).toBeNull();
    expect(screen.queryByRole("region", { name: "多语言与释义" })).toBeNull();
    // 翻译服务仍可访问，收在「更多选项」下。
    expect(within(group("翻译服务")).getByText("更多选项")).toBeTruthy();
  });

  test("saves the inverse punctuation switch and the chosen level", async () => {
    const save = await mountPhone({ sentence_association: { neural_desktop: true } });
    fireEvent.click(within(group("标点")).getByRole("switch", { name: "使用英文标点" }));
    expect((await saveAndRead(save, 0)).chinese_punctuation).toBe(false);

    fireEvent.click(within(group("智能")).getByRole("button", { name: /整句联想/ }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: /增强/ }));
    expect((await saveAndRead(save, 1)).sentence_association).toEqual({
      neural_desktop: true,
      word_lattice: true,
      neural_keyboard: true,
    });

    fireEvent.click(within(group("智能")).getByRole("button", { name: /整句联想/ }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: /关闭/ }));
    expect((await saveAndRead(save, 2)).sentence_association).toEqual({
      neural_desktop: true,
      word_lattice: false,
      neural_keyboard: false,
    });
  });

  test("the 翻译服务 row under 更多选项 offers every provider and saves the choice", async () => {
    const save = await mountPhone();
    const service = group("翻译服务");
    fireEvent.click(within(service).getByText("更多选项"));
    fireEvent.click(within(service).getByRole("button", { name: /^翻译服务/ }));
    const sheet = screen.getByRole("dialog", { name: "翻译服务" });
    for (const provider of [
      "关闭",
      "腾讯云机器翻译",
      "小牛翻译（NiuTrans）",
      "自定义 DeepLX 兼容服务",
    ]) {
      expect(within(sheet).getByRole("button", { name: provider })).toBeTruthy();
    }
    // 腾讯云默认启用，所以选小牛翻译才是一次真正的切换。
    fireEvent.click(within(sheet).getByRole("button", { name: "小牛翻译（NiuTrans）" }));
    const saved = await saveAndRead(save, 0);
    expect(saved.niutrans?.enabled).toBe(true);
    expect(saved.tencent_tmt?.enabled).toBe(false);
    expect(within(service).getByRole("button", { name: /^翻译服务/ }).textContent).toContain(
      "小牛翻译（NiuTrans）",
    );
  });

  test("an edition without the keyboard model offers no 增强", async () => {
    await mountPhone({}, { edition: { ...fullEdition, neural_keyboard: false } });
    fireEvent.click(within(group("智能")).getByRole("button", { name: /整句联想/ }));
    const sheet = screen.getByRole("dialog");
    expect(within(sheet).getByRole("button", { name: /标准/ })).toBeTruthy();
    expect(within(sheet).queryByRole("button", { name: /增强/ })).toBeNull();
  });

  test("the 2in1 keeps the desktop page", async () => {
    await mountPhone({}, { mobile_settings: false, panel_windows: true });
    expect(screen.getByRole("group", { name: "标点与翻译" })).toBeTruthy();
    expect(screen.getByRole("switch", { name: "中文标点" })).toBeTruthy();
    expect(screen.queryByRole("switch", { name: "使用英文标点" })).toBeNull();
    expect(screen.queryByRole("region", { name: "智能" })).toBeNull();
  });
});
