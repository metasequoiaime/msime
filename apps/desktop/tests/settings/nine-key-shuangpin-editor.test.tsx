// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  NineKeyShuangpinEditor,
  SettingsPage,
  type SettingsClient,
  type ShuangpinCustomProfile,
  type ShuangpinCustomProfileClient,
  type ShuangpinCustomProblem,
  type Snapshot,
} from "@msime/ui";
import { nineKeyShuangpinUnits } from "../../../../packages/ui/src/settings/nine-key-shuangpin-profile";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { testHost } from "../support/host";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** 故意让多个单位共用数字的合成表；校验由宿主替身回答，页面不能套用 26 键的一码一音规则。 */
const table: Required<ShuangpinCustomProfile> = {
  initials: Object.fromEntries(nineKeyShuangpinUnits.initials.map((unit) => [unit, "2"])),
  finals: Object.fromEntries(nineKeyShuangpinUnits.finals.map((unit) => [unit, "3"])),
  zero_initials: Object.fromEntries(
    nineKeyShuangpinUnits.zero_initials.map((unit) => [unit, "22"]),
  ),
};

type Builtin = "migala3" | "migala2" | "gushi2_low";
function fakeClient(): ShuangpinCustomProfileClient<Builtin> {
  return {
    builtin: vi.fn(async () => table),
    validate: vi.fn(
      async (draft: ShuangpinCustomProfile): Promise<ShuangpinCustomProblem | null> => {
        for (const part of ["initials", "finals", "zero_initials"] as const) {
          for (const unit of nineKeyShuangpinUnits[part]) {
            const key = draft[part]?.[unit];
            if (!key) return { kind: "missing_unit", part, unit };
            if (!(part === "zero_initials" ? /^\d{2}$/ : /^\d$/).test(key))
              return { kind: "invalid_key", part, unit, key };
          }
        }
        return null;
      },
    ),
  };
}

function button(name: string) {
  return screen.getByRole("button", { name }) as HTMLButtonElement;
}
function cell(name: string) {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}
function editor(extra: Partial<Parameters<typeof NineKeyShuangpinEditor>[0]> = {}) {
  const client = fakeClient();
  const onSave = vi.fn();
  render(
    <NineKeyShuangpinEditor
      saved={table}
      active={false}
      confirm={async () => true}
      onBack={vi.fn()}
      onSave={onSave}
      client={client}
      {...extra}
    />,
  );
  return { client, onSave };
}
function upload(profile: ShuangpinCustomProfile) {
  fireEvent.change(screen.getByLabelText("选择自定义九键双拼 JSON 文件"), {
    target: {
      files: [new File([JSON.stringify(profile)], "synthetic.json", { type: "application/json" })],
    },
  });
}

test("九键编辑器展示全部声母和数字模板，并把合法表交给保存动作", async () => {
  const { client, onSave } = editor({ saved: undefined });
  expect(cell("声母 b")).toBeTruthy();
  expect(cell("声母 zh").maxLength).toBe(1);
  expect(cell("声母 b").inputMode).toBe("numeric");
  expect(cell("零声母 eng").maxLength).toBe(2);
  fireEvent.change(screen.getByRole("combobox", { name: "内置方案" }), {
    target: { value: "gushi2_low" },
  });
  fireEvent.click(button("填入"));
  await waitFor(() => expect(cell("声母 b").value).toBe("2"));
  expect(client.builtin).toHaveBeenCalledWith("gushi2_low");
  fireEvent.change(cell("声母 b"), { target: { value: "0" } });
  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  fireEvent.click(button("保存并选用"));
  expect(onSave).toHaveBeenCalledWith({ ...table, initials: { ...table.initials, b: "0" } });
});

test("非法数字导入报告具体单元并保留已有草稿", async () => {
  const { onSave } = editor();
  upload({ ...table, zero_initials: { ...table.zero_initials, eng: "x2" } });
  expect(
    await screen.findByText("没有导入：零声母「eng」的键「x2」不能用：要两个 0–9 的数字。"),
  ).toBeTruthy();
  expect(cell("零声母 eng").value).toBe("22");
  expect(onSave).not.toHaveBeenCalled();
  upload({ ...table, finals: { ...table.finals, ing: "0" } });
  expect(await screen.findByText("已导入 synthetic.json，保存后生效。")).toBeTruthy();
  expect(cell("韵母 ing").value).toBe("0");
});

test("导出经宿主保存为独立的九键 JSON 文件", async () => {
  const saveExport = vi.fn(async (_name: string, _contents: string) => "synthetic-download.json");
  editor({ saveExport });
  await waitFor(() => expect(button("导出 JSON").disabled).toBe(false));
  fireEvent.click(button("导出 JSON"));
  await waitFor(() => expect(saveExport).toHaveBeenCalled());
  expect(saveExport.mock.calls[0][0]).toBe("msime-nine-key-shuangpin-custom.json");
  expect(JSON.parse(saveExport.mock.calls[0][1])).toEqual(table);
});

test("校验未完成不能保存，迟到的旧结果不能覆盖新数字表", async () => {
  let finish!: (value: ShuangpinCustomProblem | null) => void;
  const client = fakeClient();
  const old = new Promise<ShuangpinCustomProblem | null>((resolve) => {
    finish = resolve;
  });
  const validate = vi.spyOn(client, "validate").mockReturnValueOnce(old);
  editor({ client });
  await waitFor(() => expect(validate).toHaveBeenCalledTimes(1));
  expect(button("保存并选用").disabled).toBe(true);
  fireEvent.change(cell("声母 b"), { target: { value: "0" } });
  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  await act(async () => {
    finish({ kind: "missing_unit", part: "initials", unit: "b" });
    await old;
  });
  expect(button("保存并选用").disabled).toBe(false);
  expect(screen.queryByText("声母「b」还没有指定键。")).toBeNull();
});

async function renderSettings(withReader = true) {
  let snapshot: Snapshot = {
    format_version: 1,
    revision: 7,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "custom",
      shuangpin_custom_profile: { initials: { zh: "v" } },
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      touch_keyboard_layout: "nine_key_shuangpin",
      touch_keyboard_schemes: {
        enabled: ["quanpin", "nine_key_shuangpin"],
        selected: "nine_key_shuangpin",
      },
    },
  };
  const savedLetter = snapshot.preferences.shuangpin_custom_profile;
  const save = vi.fn(
    async (_revision: number, preferences: Snapshot["preferences"]) =>
      (snapshot = { ...snapshot, revision: snapshot.revision + 1, preferences }),
  );
  const reader = vi.fn<NonNullable<SettingsClient["nineKeyShuangpinLayout"]>>(async (request) => ({
    effective: request.profile,
    table: request.table ?? table,
    problem: null,
  }));
  render(
    <SettingsPage
      initialPage="input"
      client={{
        host: testHost({ platform: "harmony" }),
        touchKeyboardSchemes: true,
        load: async () => snapshot,
        save,
        nineKeyShuangpinLayout: withReader ? reader : undefined,
      }}
    />,
  );
  await settingsFormReady();
  return { save, reader, savedLetter };
}

test("鸿蒙设置选内置方案和保存自定义表只改九键字段", async () => {
  const { save, reader, savedLetter } = await renderSettings();
  fireEvent.click(screen.getByRole("button", { name: /^九键双拼方案/ }));
  fireEvent.click(button("米旮旯 2"));
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      expect.any(Number),
      expect.objectContaining({ nine_key_shuangpin_profile: "migala2" }),
    ),
  );
  fireEvent.click(button("编辑自定义九键双拼"));
  fireEvent.click(button("填入"));
  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  fireEvent.click(button("保存并选用"));
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(
      expect.any(Number),
      expect.objectContaining({
        nine_key_shuangpin_profile: "custom",
        nine_key_shuangpin_custom_profile: table,
        shuangpin_profile: "custom",
        shuangpin_custom_profile: savedLetter,
      }),
    ),
  );
  expect(reader).toHaveBeenCalledWith({ profile: "custom", table });
});

test("没有宿主布局校验能力时不显示编辑入口", async () => {
  await renderSettings(false);
  expect(screen.queryByRole("button", { name: "编辑自定义九键双拼" })).toBeNull();
});
