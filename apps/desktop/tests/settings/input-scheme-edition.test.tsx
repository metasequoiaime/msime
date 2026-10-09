// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type EditionInfo, type HostCapabilities, type Snapshot } from "@msime/ui";
import {
  editionDefaultChineseScheme,
  editionUsesHelpcode,
  fallbackChineseScheme,
  singleEditionScheme,
} from "../../../../packages/ui/src/settings/input-scheme-options";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// 与 client-core 的 `HostCapabilities::narrow_to_edition` 对五笔版、拼音版给出的结果一致。
const wubiEdition: EditionInfo = {
  id: "wubi",
  input_schemes: ["wubi"],
  default_scheme: "wubi",
  temporary_japanese: false,
  neural_keyboard: false,
  offline_glosses: true,
  handwriting: true,
  wubi_mixed_pinyin_default: true,
};

const pinyinEdition: EditionInfo = {
  id: "pinyin",
  input_schemes: ["quanpin", "shuangpin"],
  default_scheme: "quanpin",
  temporary_japanese: true,
  neural_keyboard: true,
  offline_glosses: true,
  handwriting: true,
  wubi_mixed_pinyin_default: false,
};

const fullHost = testHost({ platform: "windows" });
const wubiHost = testHost({ platform: "windows", input_schemes: ["wubi"], edition: wubiEdition });
const pinyinHost = testHost({
  platform: "windows",
  input_schemes: ["quanpin", "shuangpin"],
  edition: pinyinEdition,
});

function snapshot(preferences: Partial<Snapshot["preferences"]> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  };
}

async function openInputPage(
  host: HostCapabilities,
  preferences?: Partial<Snapshot["preferences"]>,
) {
  render(
    <SettingsPage
      client={{ load: async () => snapshot(preferences), save: vi.fn(), host, fuzzyPinyin: true }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  await screen.findByRole("switch", { name: "启用模糊音" });
}

test("full keeps every scheme, the input modes, helper codes and 临时日语", async () => {
  await openInputPage(fullHost);

  for (const name of ["全拼", "双拼", "五笔", "粤拼", "注音", "笔画"]) {
    expect(screen.getByRole("radio", { name })).toBeTruthy();
  }
  expect(screen.getByRole("radio", { name: "中文" })).toBeTruthy();
  expect(screen.getByText("此平台暂不支持粤拼、注音、笔画")).toBeTruthy();
  expect(
    screen.getByText(
      "切换中文、日文、韩文、越南文或藏文输入，并保留各模式上次选择的方案；此平台暂不支持越南文、藏文",
    ),
  ).toBeTruthy();
  expect(screen.getByRole("region", { name: "辅助码" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: /^临时日语/ })).toBeTruthy();
  // 全拼时不显示五笔的设置。
  expect(screen.queryByRole("switch", { name: "五笔拼音混输" })).toBeNull();
});

test("the wubi edition offers no other scheme and always shows the Wubi settings", async () => {
  // 账号同步或别的版本带来的文档写着全拼，混拼也没有记录。
  await openInputPage(wubiHost, { scheme: "quanpin", last_chinese_scheme: "quanpin" });

  // 本版本没有的方案直接不列出，也不说「此平台暂不支持」；只剩一个方案，方案选择和输入模式都不显示。
  for (const name of [
    "全拼",
    "双拼",
    "五笔",
    "粤拼",
    "注音",
    "笔画",
    "中文",
    "日文",
    "韩文",
    "越南文",
    "藏文",
  ]) {
    expect(screen.queryByRole("radio", { name })).toBeNull();
  }
  expect(screen.queryByText(/此平台暂不支持/)).toBeNull();
  const mixed = screen.getByRole("switch", { name: "五笔拼音混输" }) as HTMLInputElement;
  expect(mixed.checked).toBe(true);
  // 五笔不用辅助码，这一组不显示；模糊音在混拼查全拼时照常起作用。
  expect(screen.queryByRole("region", { name: "辅助码" })).toBeNull();
  expect(screen.getByRole("switch", { name: "启用模糊音" })).toBeTruthy();
  // 五笔版不带临时日文。
  expect(screen.queryByRole("switch", { name: /^临时日语/ })).toBeNull();
  expect(screen.getByRole("switch", { name: /^临时英文/ })).toBeTruthy();
});

test("the wubi edition keeps a mixed-pinyin choice the user turned off", async () => {
  await openInputPage(wubiHost, { scheme: "wubi", wubi_mixed_pinyin: false });

  const mixed = screen.getByRole("switch", { name: "五笔拼音混输" }) as HTMLInputElement;
  expect(mixed.checked).toBe(false);
});

test("the pinyin edition lists only its two schemes and names the fallback for a Wubi document", async () => {
  await openInputPage(pinyinHost, { scheme: "wubi", last_chinese_scheme: "wubi" });

  expect(screen.getByRole("radio", { name: "全拼" })).toBeTruthy();
  expect(screen.getByRole("radio", { name: "双拼" })).toBeTruthy();
  for (const name of ["五笔", "粤拼", "注音", "笔画"]) {
    expect(screen.queryByRole("radio", { name })).toBeNull();
  }
  expect(screen.getByText("已回退到全拼")).toBeTruthy();
  // 拼音版没有日文、韩文、越南文方案，输入模式只剩「中文」，不显示。
  expect(screen.queryByRole("radio", { name: "中文" })).toBeNull();
  expect(screen.getByRole("region", { name: "辅助码" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: /^临时日语/ })).toBeTruthy();
});

test("a pinyin-edition document naming Japanese shows the scheme host-api runs", async () => {
  await openInputPage(pinyinHost, { scheme: "japanese", last_chinese_scheme: "shuangpin" });

  expect((screen.getByRole("radio", { name: "双拼" }) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "全拼" }));
  expect((screen.getByRole("radio", { name: "全拼" }) as HTMLInputElement).checked).toBe(true);
});

test("edition helpers fall back to the full answers without an edition", () => {
  expect(editionDefaultChineseScheme()).toBe("quanpin");
  expect(editionDefaultChineseScheme(wubiEdition)).toBe("wubi");
  expect(singleEditionScheme()).toBeUndefined();
  expect(singleEditionScheme(pinyinEdition)).toBeUndefined();
  expect(singleEditionScheme(wubiEdition)).toBe("wubi");
  expect(editionUsesHelpcode()).toBe(true);
  expect(editionUsesHelpcode(pinyinEdition)).toBe(true);
  expect(editionUsesHelpcode(wubiEdition)).toBe(false);
  expect(fallbackChineseScheme("cantonese", ["quanpin"])).toBe("quanpin");
  expect(fallbackChineseScheme("quanpin", ["wubi"], "wubi")).toBe("wubi");
});

test("a touch host lists 键盘神经联想 only when its edition ships the keyboard model", async () => {
  await openInputPage(testHost({ platform: "android" }));
  expect(screen.getByRole("switch", { name: "键盘神经联想" })).toBeTruthy();
  cleanup();

  await openInputPage(
    testHost({ platform: "android", input_schemes: ["wubi"], edition: wubiEdition }),
  );
  expect(screen.queryByRole("switch", { name: "键盘神经联想" })).toBeNull();
  expect(screen.getByRole("switch", { name: "本地整句联想" })).toBeTruthy();
});

test("the desktop 桌面神经联想 switch is not tied to the keyboard model", async () => {
  await openInputPage(wubiHost);
  expect(screen.getByRole("switch", { name: "桌面神经联想" })).toBeTruthy();
});

test("a touch host offers 手写 only in an edition with a Chinese scheme", async () => {
  const vietnameseEdition: EditionInfo = {
    id: "vietnamese",
    input_schemes: ["vietnamese"],
    default_scheme: "vietnamese",
    temporary_japanese: false,
    neural_keyboard: false,
    offline_glosses: false,
    handwriting: false,
    wubi_mixed_pinyin_default: false,
  };
  async function touchSchemes(host: HostCapabilities, scheme: Snapshot["preferences"]["scheme"]) {
    render(
      <SettingsPage
        client={{
          load: async () => snapshot({ scheme }),
          save: vi.fn(),
          host,
          touchKeyboardSchemes: true,
        }}
      />,
    );
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: "输入" }));
    await screen.findByRole("group", { name: "输入方案" });
  }

  // 手写识别器只认汉字：越南文版的键盘只有越南语，没有手写。
  await touchSchemes(
    testHost({ platform: "android", input_schemes: ["vietnamese"], edition: vietnameseEdition }),
    "vietnamese",
  );
  expect(screen.getByRole("switch", { name: "显示输入方案 越南语 26 键" })).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "显示输入方案 手写" })).toBeNull();
  cleanup();

  await touchSchemes(
    testHost({ platform: "android", input_schemes: ["wubi"], edition: wubiEdition }),
    "wubi",
  );
  expect(screen.getByRole("switch", { name: "显示输入方案 手写" })).toBeTruthy();
});

// 手写只认汉字：没有中文方案的版本（`EditionInfo.handwriting` 为 false）设置导航里没有「手写输入」页，中文的版本照旧有。
test("the settings navigation lists 手写输入 only in an edition with handwriting", async () => {
  const japaneseEdition: EditionInfo = {
    id: "japanese",
    input_schemes: ["japanese"],
    default_scheme: "japanese",
    temporary_japanese: false,
    neural_keyboard: false,
    offline_glosses: false,
    handwriting: false,
    wubi_mixed_pinyin_default: false,
  };
  async function open(host: HostCapabilities, scheme: Snapshot["preferences"]["scheme"]) {
    render(
      <SettingsPage client={{ load: async () => snapshot({ scheme }), save: vi.fn(), host }} />,
    );
    await settingsFormReady();
  }

  await open(
    testHost({ platform: "windows", input_schemes: ["japanese"], edition: japaneseEdition }),
    "japanese",
  );
  expect(screen.queryByRole("button", { name: "手写输入" })).toBeNull();
  expect(screen.getByRole("button", { name: "语音输入" })).toBeTruthy();
  cleanup();

  await open(wubiHost, "wubi");
  expect(screen.getByRole("button", { name: "手写输入" })).toBeTruthy();
});
