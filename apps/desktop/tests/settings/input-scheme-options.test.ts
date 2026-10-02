import { testHost } from "../support/host";
import { expect, test } from "vitest";
import {
  cantoneseInputSchemeOptions,
  chineseInputSchemeOptions,
  fallbackChineseScheme,
  isChineseScheme,
  japaneseInputSchemeOptions,
  koreanInputSchemeOptions,
  nonChineseSchemes,
  supportedInputSchemes,
  vietnameseInputMethodOptions,
  vietnameseToneStyleOptions,
  zhuyinLayoutOptions,
} from "../../../../packages/ui/src/settings/input-scheme-options";
import type { InputScheme } from "@msime/ui";

test("shares the input scheme labels used by settings controls", () => {
  expect(chineseInputSchemeOptions).toEqual([
    { value: "quanpin", label: "全拼" },
    { value: "shuangpin", label: "双拼" },
    { value: "wubi", label: "五笔" },
    { value: "cantonese", label: "粤拼" },
    { value: "zhuyin", label: "注音" },
  ]);
  expect(japaneseInputSchemeOptions).toEqual([{ value: "romaji", label: "罗马音" }]);
  expect(koreanInputSchemeOptions).toEqual([{ value: "dubeolsik", label: "两套式" }]);
  expect(cantoneseInputSchemeOptions).toEqual([{ value: "jyutping", label: "粤拼" }]);
  expect(zhuyinLayoutOptions).toEqual([{ value: "dachen", label: "大千" }]);
  expect(vietnameseInputMethodOptions).toEqual([
    { value: "telex", label: "Telex" },
    { value: "vni", label: "VNI" },
  ]);
  expect(vietnameseToneStyleOptions).toEqual([
    { value: "modern", label: "新式 hoà" },
    { value: "classic", label: "旧式 hòa" },
  ]);
});

test("Cantonese and Zhuyin are Chinese schemes and Vietnamese is a mode of its own", () => {
  expect(nonChineseSchemes).toEqual(["japanese", "korean", "vietnamese"]);
  const chinese = (
    [
      "quanpin",
      "shuangpin",
      "wubi",
      "japanese",
      "korean",
      "cantonese",
      "zhuyin",
      "vietnamese",
    ] as const
  ).filter((scheme) => isChineseScheme(scheme));
  expect(chinese).toEqual(["quanpin", "shuangpin", "wubi", "cantonese", "zhuyin"]);
});

test("without a host the five base schemes are offered", () => {
  expect(supportedInputSchemes()).toEqual(["quanpin", "shuangpin", "wubi", "japanese", "korean"]);
});

test("input_schemes values the page has no label for are dropped", () => {
  const host = testHost({
    platform: "macos",
    input_schemes: ["quanpin", "cantonese", "klingon" as InputScheme, "vietnamese"],
  });
  expect(supportedInputSchemes(host)).toEqual(["quanpin", "cantonese", "vietnamese"]);
});

test("the fallback is the remembered Chinese scheme when offered, else 全拼", () => {
  const base = supportedInputSchemes();
  expect(fallbackChineseScheme("wubi", base)).toBe("wubi");
  expect(fallbackChineseScheme("cantonese", base)).toBe("quanpin");
  expect(fallbackChineseScheme(null, base)).toBe("quanpin");
  expect(fallbackChineseScheme("zhuyin", [...base, "zhuyin"])).toBe("zhuyin");
});
