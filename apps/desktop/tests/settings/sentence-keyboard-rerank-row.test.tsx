// @vitest-environment jsdom
import { testHost } from "../support/host";
import { selectSettingsTab, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type EditionInfo, type Snapshot } from "@msime/ui";
import { SentenceAssociationSection } from "../../../../packages/ui/src/settings/sentence-association-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const levelSelect = () => screen.getByRole("combobox", { name: /^整句联想/ }) as HTMLSelectElement;

test("桌面输入页的「整句联想」与手机同名，只列标准和增强，缺键文档显示默认的增强", () => {
  render(<SentenceAssociationSection value={undefined} mobile={false} onChange={vi.fn()} />);
  expect(Array.from(levelSelect().options).map((option) => option.textContent)).toEqual([
    "标准",
    "增强",
  ]);
  expect(levelSelect().value).toBe("2");
  // 与「桌面神经联想」是两行：一个是逐键重排，一个是停顿重排。
  expect(screen.getByRole("switch", { name: "桌面神经联想" })).toBeTruthy();
});

test("选标准只写 neural_keyboard，保留整句联想里的其他字段；显式的标准显示为标准", () => {
  const onChange = vi.fn();
  const value = { word_lattice: true, neural_desktop: true, show_next_on_duplicate: false };
  const { rerender } = render(
    <SentenceAssociationSection value={value} mobile={false} onChange={onChange} />,
  );
  fireEvent.change(levelSelect(), { target: { value: "1" } });
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenCalledWith({ ...value, neural_keyboard: false });

  rerender(
    <SentenceAssociationSection
      value={{ ...value, neural_keyboard: false }}
      mobile={false}
      onChange={onChange}
    />,
  );
  expect(levelSelect().value).toBe("1");
  fireEvent.change(levelSelect(), { target: { value: "2" } });
  expect(onChange).toHaveBeenLastCalledWith({ ...value, neural_keyboard: true });
});

test("不带逐键重排模型的版本（五笔版）在桌面上不列出「整句联想」；触屏外壳不列出它而是键盘神经联想开关", () => {
  render(
    <SentenceAssociationSection
      value={undefined}
      mobile={false}
      neuralKeyboard={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("combobox", { name: /^整句联想/ })).toBeNull();
  expect(screen.getByRole("switch", { name: "桌面神经联想" })).toBeTruthy();
  cleanup();

  render(<SentenceAssociationSection value={undefined} mobile onChange={vi.fn()} />);
  expect(screen.queryByRole("combobox", { name: /^整句联想/ })).toBeNull();
  // 触屏外壳上缺键文档的键盘神经联想按默认的开显示。
  expect((screen.getByRole("switch", { name: "键盘神经联想" }) as HTMLInputElement).checked).toBe(
    true,
  );
});

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

test.each(["macos", "linux"] as const)(
  "%s 的输入页在「候选与联想」组列出「整句联想」，五笔版不列",
  async (platform) => {
    const wubiEdition: EditionInfo = {
      ...fullEdition,
      id: "wubi",
      input_schemes: ["wubi"],
      default_scheme: "wubi",
      neural_keyboard: false,
    };
    const cases: [EditionInfo, boolean][] = [
      [fullEdition, true],
      [wubiEdition, false],
    ];
    for (const [edition, shown] of cases) {
      render(
        <SettingsPage
          client={{
            load: async () => snapshot({ scheme: edition.default_scheme }),
            save: vi.fn(),
            host: testHost({ platform, input_schemes: [...edition.input_schemes], edition }),
          }}
        />,
      );
      await settingsFormReady();
      selectSettingsTab("候选与联想");
      await screen.findByRole("switch", { name: "桌面神经联想" });
      const row = screen.queryByRole("combobox", { name: /^整句联想/ }) as HTMLSelectElement | null;
      expect(Boolean(row), `${platform} ${edition.id}`).toBe(shown);
      if (row) expect(row.value).toBe("2");
      cleanup();
    }
  },
);
