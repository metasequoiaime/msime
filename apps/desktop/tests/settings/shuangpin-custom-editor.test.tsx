// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  InputSchemeDetailsSection,
  SettingsPage,
  ShuangpinCustomEditor,
  parseShuangpinCustomProfile,
  type ShuangpinCustomProblem,
  type ShuangpinCustomProfile,
  type ShuangpinCustomProfileClient,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** 小鹤双拼的键位（合成数据，与内置方案相同）。 */
const xiaohe = {
  initials: { zh: "v", ch: "i", sh: "u" },
  finals: {
    iu: "q",
    ei: "w",
    e: "e",
    uan: "r",
    ue: "t",
    ve: "t",
    un: "y",
    u: "u",
    i: "i",
    uo: "o",
    o: "o",
    ie: "p",
    a: "a",
    ong: "s",
    iong: "s",
    ai: "d",
    en: "f",
    eng: "g",
    ang: "h",
    an: "j",
    uai: "k",
    ing: "k",
    uang: "l",
    iang: "l",
    ou: "z",
    ua: "x",
    ia: "x",
    ao: "c",
    ui: "v",
    v: "v",
    in: "b",
    iao: "n",
    ian: "m",
  },
  zero_initials: {
    a: "aa",
    ai: "ai",
    an: "an",
    ang: "ah",
    ao: "ao",
    e: "ee",
    ei: "ei",
    en: "en",
    eng: "eg",
    er: "er",
    o: "oo",
    ou: "ou",
  },
} satisfies ShuangpinCustomProfile;

const parts = ["initials", "finals", "zero_initials"] as const;

/** 宿主的替身：缺键报 missing_unit，ang 放到 k 上报编码冲突（小鹤的 uai 和 ing 已在 k 上），其余都算能用。 */
function fakeClient(): ShuangpinCustomProfileClient & {
  validate: ReturnType<typeof vi.fn>;
  builtin: ReturnType<typeof vi.fn>;
} {
  return {
    validate: vi.fn(
      async (profile: ShuangpinCustomProfile): Promise<ShuangpinCustomProblem | null> => {
        for (const part of parts) {
          const unit = Object.keys(xiaohe[part]).find((name) => !profile[part]?.[name]);
          if (unit) return { kind: "missing_unit", part, unit };
        }
        if (profile.finals?.ang === "k")
          return { kind: "ambiguous_code", code: "hk", first: "hang", second: "huai" };
        return null;
      },
    ),
    builtin: vi.fn(async (name: string) => (name === "xiaohe" ? xiaohe : null)),
  };
}

function cell(name: string): HTMLInputElement {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}

function button(name: string): HTMLButtonElement {
  return screen.getByRole("button", { name }) as HTMLButtonElement;
}

function renderEditor(
  props: Partial<Parameters<typeof ShuangpinCustomEditor>[0]> & {
    client: ShuangpinCustomProfileClient;
  },
) {
  const onSave = vi.fn();
  const onBack = vi.fn();
  const confirm = vi.fn(async () => true);
  render(
    <ShuangpinCustomEditor
      saved={undefined}
      active={false}
      confirm={confirm}
      onSave={onSave}
      onBack={onBack}
      {...props}
    />,
  );
  return { onSave, onBack, confirm };
}

function upload(text: string, name = "table.json") {
  const input = screen.getByLabelText("选择自定义双拼 JSON 文件") as HTMLInputElement;
  fireEvent.change(input, {
    target: { files: [new File([text], name, { type: "application/json" })] },
  });
}

test("fills an empty table from a built-in profile and saves it as the custom profile", async () => {
  const client = fakeClient();
  const { onSave, confirm } = renderEditor({ client });

  expect(await screen.findByText("声母「zh」还没有指定键。")).toBeTruthy();
  expect(button("保存并选用").disabled).toBe(true);

  fireEvent.click(button("填入"));
  await waitFor(() => expect(cell("韵母 ing").value).toBe("k"));
  // 空表不用确认就能填入。
  expect(confirm).not.toHaveBeenCalled();
  expect(client.builtin).toHaveBeenCalledWith("xiaohe");
  expect(cell("声母 zh").value).toBe("v");
  expect(cell("零声母 eng").value).toBe("eg");

  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  fireEvent.click(button("保存并选用"));
  expect(onSave).toHaveBeenCalledWith(xiaohe);
});

test("names the problem the host reports and blocks saving until it is fixed", async () => {
  const client = fakeClient();
  const { onSave } = renderEditor({ client, saved: xiaohe, active: true });

  // 已经在用、没有改动时没有可保存的东西。
  await waitFor(() => expect(client.validate).toHaveBeenCalled());
  expect(button("保存").disabled).toBe(true);

  fireEvent.change(cell("韵母 ang"), { target: { value: "K" } });
  expect(
    await screen.findByText("编码「hk」既是「hang」又是「huai」，两个音节不能用同一个编码。"),
  ).toBeTruthy();
  expect(cell("韵母 ang").value).toBe("k");
  expect(button("保存").disabled).toBe(true);

  fireEvent.change(cell("韵母 ang"), { target: { value: "" } });
  expect(await screen.findByText("韵母「ang」还没有指定键。")).toBeTruthy();
  expect(cell("韵母 ang").getAttribute("aria-invalid")).toBe("true");

  fireEvent.change(cell("韵母 ang"), { target: { value: "h" } });
  fireEvent.change(cell("韵母 ing"), { target: { value: ";" } });
  await waitFor(() => expect(button("保存").disabled).toBe(false));
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(button("保存"));
  expect(onSave).toHaveBeenCalledWith({ ...xiaohe, finals: { ...xiaohe.finals, ing: ";" } });
});

test("an import that is not a usable table is refused and leaves the draft alone", async () => {
  const client = fakeClient();
  renderEditor({ client, saved: xiaohe, active: true });

  upload("{ not json");
  expect(await screen.findByText("没有导入：文件不是有效的 JSON。")).toBeTruthy();

  upload(JSON.stringify({ ...xiaohe, version: 1 }));
  expect(
    await screen.findByText(
      "没有导入：文件里有不认识的部分「version」，只能有 initials、finals、zero_initials。",
    ),
  ).toBeTruthy();

  upload(JSON.stringify({ ...xiaohe, finals: { ...xiaohe.finals, ang: "k" } }));
  expect(
    await screen.findByText(
      "没有导入：编码「hk」既是「hang」又是「huai」，两个音节不能用同一个编码。",
    ),
  ).toBeTruthy();
  expect(cell("韵母 ang").value).toBe("h");

  upload(JSON.stringify({ ...xiaohe, finals: { ...xiaohe.finals, ing: ";" } }), "mine.json");
  expect(await screen.findByText("已导入 mine.json，保存后生效。")).toBeTruthy();
  expect(cell("韵母 ing").value).toBe(";");
});

test("exports the table through the host and replaces edits only after a confirmation", async () => {
  const client = fakeClient();
  const saveExport = vi.fn(async () => "/synthetic/Downloads/msime-shuangpin-custom.json");
  const { confirm } = renderEditor({ client, saved: xiaohe, active: true, saveExport });

  await waitFor(() => expect(button("导出 JSON").disabled).toBe(false));
  fireEvent.click(button("导出 JSON"));
  expect(
    await screen.findByText("已导出到 /synthetic/Downloads/msime-shuangpin-custom.json"),
  ).toBeTruthy();
  const [[name, contents]] = saveExport.mock.calls as unknown as [[string, string]];
  expect(name).toBe("msime-shuangpin-custom.json");
  // 导出的文件能原样导入回来。
  expect(parseShuangpinCustomProfile(contents)).toEqual(xiaohe);

  fireEvent.click(button("填入"));
  await waitFor(() => expect(confirm).toHaveBeenCalledTimes(1));
});

test("the 自定义双拼 option follows the host's ability to edit it", () => {
  const onShuangpinProfileChange = vi.fn();
  const onEditCustomShuangpin = vi.fn();
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );
  const profile = () => screen.getByRole("combobox", { name: "双拼方案" }) as HTMLSelectElement;
  // 宿主编辑不了时不列出它，也没有编辑入口。
  expect(Array.from(profile().options).map((option) => option.value)).not.toContain("custom");
  expect(screen.queryByRole("button", { name: "编辑自定义键位" })).toBeNull();

  // 文档里已经是自定义双拼：照样列出来，选择框才显示得对，但不能再选。
  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="custom"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );
  expect(profile().value).toBe("custom");
  expect(Array.from(profile().options).find((option) => option.value === "custom")?.disabled).toBe(
    true,
  );
  expect(screen.queryByRole("button", { name: "编辑自定义键位" })).toBeNull();

  // 能编辑时，选它先打开编辑页，不直接改方案。
  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
      onEditCustomShuangpin={onEditCustomShuangpin}
    />,
  );
  fireEvent.change(profile(), { target: { value: "custom" } });
  expect(onEditCustomShuangpin).toHaveBeenCalledTimes(1);
  expect(onShuangpinProfileChange).not.toHaveBeenCalled();

  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="custom"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
      onEditCustomShuangpin={onEditCustomShuangpin}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "编辑自定义键位" }));
  expect(onEditCustomShuangpin).toHaveBeenCalledTimes(2);
});

function snapshot(preferences: Partial<Snapshot["preferences"]> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "shuangpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  };
}

test("the input page saves only a usable table and switches to it", async () => {
  const shuangpin = fakeClient();
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...snapshot(),
    revision: 4,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => snapshot(),
        save,
        host: testHost({ platform: "linux" }),
        validateShuangpinCustomProfile: shuangpin.validate,
        builtinShuangpinTable: shuangpin.builtin,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const profile = (await screen.findByRole("combobox", { name: "双拼方案" })) as HTMLSelectElement;

  fireEvent.change(profile, { target: { value: "custom" } });
  expect(await screen.findByRole("heading", { name: "自定义双拼" })).toBeTruthy();
  fireEvent.click(button("填入"));
  await waitFor(() => expect(cell("韵母 ing").value).toBe("k"));
  fireEvent.change(cell("韵母 ing"), { target: { value: ";" } });
  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  fireEvent.click(button("保存并选用"));

  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      3,
      expect.objectContaining({
        shuangpin_profile: "custom",
        shuangpin_custom_profile: { ...xiaohe, finals: { ...xiaohe.finals, ing: ";" } },
      }),
    ),
  );

  // 返回后选择框显示自定义双拼，下面有编辑入口。
  fireEvent.click(screen.getByRole("button", { name: "返回输入" }));
  await waitFor(() =>
    expect((screen.getByRole("combobox", { name: "双拼方案" }) as HTMLSelectElement).value).toBe(
      "custom",
    ),
  );
  expect(screen.getByRole("button", { name: "编辑自定义键位" })).toBeTruthy();
});

test("leaving the editor with unsaved edits asks first", async () => {
  const client = fakeClient();
  const confirm = vi.fn(async () => false);
  const { onBack } = renderEditor({ client, saved: xiaohe, active: true, confirm });
  fireEvent.change(cell("韵母 ing"), { target: { value: ";" } });
  fireEvent.click(screen.getByRole("button", { name: "返回输入" }));
  await waitFor(() => expect(confirm).toHaveBeenCalledTimes(1));
  expect(onBack).not.toHaveBeenCalled();
});

test("going to another settings page keeps the unsaved table in the editor", async () => {
  const shuangpin = fakeClient();
  render(
    <SettingsPage
      client={{
        load: async () => snapshot(),
        save: vi.fn(),
        host: testHost({ platform: "linux" }),
        validateShuangpinCustomProfile: shuangpin.validate,
        builtinShuangpinTable: shuangpin.builtin,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const profile = (await screen.findByRole("combobox", { name: "双拼方案" })) as HTMLSelectElement;
  fireEvent.change(profile, { target: { value: "custom" } });
  fireEvent.click(await screen.findByRole("button", { name: "填入" }));
  await waitFor(() => expect(cell("韵母 ing").value).toBe("k"));
  fireEvent.change(cell("韵母 ing"), { target: { value: ";" } });

  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  fireEvent.click(screen.getByRole("button", { name: "输入" }));

  expect(await screen.findByRole("heading", { name: "自定义双拼" })).toBeTruthy();
  expect(cell("韵母 ing").value).toBe(";");
});

test("without host support the input page offers no custom shuangpin", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot(),
        save: vi.fn(),
        host: testHost({ platform: "linux" }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const profile = (await screen.findByRole("combobox", { name: "双拼方案" })) as HTMLSelectElement;
  expect(Array.from(profile.options).map((option) => option.value)).not.toContain("custom");
  expect(screen.queryByRole("button", { name: "编辑自定义键位" })).toBeNull();
});

test("a host that cannot answer leaves saving off and asks again after the next edit", async () => {
  const client = fakeClient();
  client.validate.mockRejectedValueOnce(new Error("bridge"));
  renderEditor({ client, saved: xiaohe, active: false });
  expect(await screen.findByText("无法校验键位表，改动任意一个键后会重新校验。")).toBeTruthy();
  expect(button("保存并选用").disabled).toBe(true);

  fireEvent.change(cell("韵母 ing"), { target: { value: ";" } });
  await waitFor(() => expect(button("保存并选用").disabled).toBe(false));
  expect(screen.queryByText("无法校验键位表，改动任意一个键后会重新校验。")).toBeNull();
});
