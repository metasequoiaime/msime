// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  PluginsSection,
  ScriptGrantDialog,
  defaultPluginPreferences,
  toggleCapability,
  type PluginClient,
  type PluginPackage,
  type ScriptCapability,
  type ScriptGrantsResult,
  type ScriptPluginState,
} from "@msime/ui";
import { CommunityPluginCapabilities } from "../../../../packages/ui/src/community/community-plugin-capabilities";
import {
  PERSIST_DATA_WARNING,
  capabilityDescription,
  formatKilobytes,
  grantDeletesStoredData,
} from "../../../../packages/ui/src/settings/plugin-script-model";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const HASH = "0123456789abcdef".repeat(4);

const script = (overrides: Partial<PluginPackage> & Pick<PluginPackage, "id">): PluginPackage =>
  ({
    name: overrides.id,
    version: "1.0.0",
    license: "MIT",
    author: null,
    description: null,
    builtin: false,
    kind: "script",
    abi: 1,
    module: "main.wasm",
    hooks: ["candidates", "commit"],
    permissions: [],
    memory_pages: 16,
    content_hash: HASH,
    ...overrides,
  }) as PluginPackage;

// 合成的插件：记住常用表情，往候选里加，带四种设置项和随包数据。
const memo = script({
  id: "memo",
  name: "常用表情",
  permissions: ["read_commit", "read_candidates", "insert_candidates", "persist_data"],
  data: ["words.bin"],
  data_bytes: 5000,
  settings: [
    { key: "hide", title: "隐藏生僻字", description: "打开后不显示", type: "bool", default: true },
    {
      key: "level",
      title: "字表",
      type: "choice",
      options: [
        { value: "level_1", title: "一级字" },
        { value: "level_2", title: "二级字" },
      ],
      default: "level_2",
    },
    { key: "threshold", title: "阈值", type: "number", min: 0, max: 9, default: 3 },
    { key: "suffix", title: "后缀", type: "text", max_length: 4, default: "" },
  ],
});

const defaults = { hide: true, level: "level_2", threshold: 3, suffix: "" };

function grants(overrides: Partial<ScriptGrantsResult> = {}): ScriptGrantsResult {
  return { revision: 1, plugins: [], health: [], commands: [], ...overrides };
}

const persisting = grants({
  plugins: [
    {
      id: "memo",
      hash: HASH,
      enabled: true,
      granted: ["read_commit", "read_candidates", "insert_candidates", "persist_data"],
    },
  ],
});

function pluginState(overrides: Partial<ScriptPluginState> = {}): ScriptPluginState {
  return { id: "memo", values: defaults, storage_bytes: 0, ...overrides };
}

function fakeClient(overrides: Partial<PluginClient> = {}): PluginClient {
  return {
    catalog: vi.fn(async () => ({ packages: [memo], issues: [] })),
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => undefined),
    loadMentions: vi.fn(async () => []),
    saveMentions: vi.fn(async () => undefined),
    scriptGrants: vi.fn(async () => grants()),
    setScriptGrant: vi.fn(async () => grants()),
    reorderScripts: vi.fn(async () => grants()),
    scriptPluginState: vi.fn(async () => pluginState()),
    setScriptSettings: vi.fn(async (_id, values) => pluginState({ values })),
    clearScriptData: vi.fn(async () => pluginState()),
    ...overrides,
  };
}

function renderSection(client: PluginClient, confirm = vi.fn(async () => true)) {
  render(
    <PluginsSection
      client={client}
      preferences={defaultPluginPreferences}
      keySound
      music
      triggers
      active
      scriptPlugins
      platform="macos"
      onChange={vi.fn()}
      onError={vi.fn()}
      confirm={confirm}
    />,
  );
  return { confirm };
}

async function openMemo() {
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(await within(list).findByRole("button", { name: "常用表情" }));
  await screen.findByRole("heading", { name: "常用表情" });
}

const check = (name: string) =>
  screen.getByRole<HTMLInputElement>("checkbox", { name: new RegExp(`^${name}`) });

test("adding candidates needs reading them, and keeping data never goes with rewriting the commit", () => {
  const requested: ScriptCapability[] = [
    "read_commit",
    "read_candidates",
    "rewrite_commit",
    "insert_candidates",
    "persist_data",
  ];
  expect(toggleCapability([], "insert_candidates", true, requested)).toEqual([
    "read_candidates",
    "insert_candidates",
  ]);
  expect(
    toggleCapability(["read_candidates", "insert_candidates"], "read_candidates", false, requested),
  ).toEqual([]);
  // 勾选改写时，长期保存数据让开；反过来也一样，改写依赖的读上屏留着。
  expect(toggleCapability(["persist_data"], "rewrite_commit", true, requested)).toEqual([
    "read_commit",
    "rewrite_commit",
  ]);
  expect(
    toggleCapability(["read_commit", "rewrite_commit"], "persist_data", true, requested),
  ).toEqual(["read_commit", "persist_data"]);
});

test("a pack asking for the new capabilities lists them in groups, with keeping data alone, unticked and in red", () => {
  render(
    <ScriptGrantDialog
      pack={memo}
      initial={[]}
      stale
      onConfirm={vi.fn(async () => undefined)}
      onCancel={vi.fn()}
    />,
  );
  // 改之前 `sortCapabilities` 把不认识的两项悄悄去掉，对话框里根本没有它们。
  for (const [legend, names] of [
    ["读文字", ["读取输入的文字", "读取候选词"]],
    ["改文字", ["添加候选"]],
    ["记住数据", ["长期保存数据"]],
  ] as const) {
    const group = screen.getByRole("group", { name: legend });
    expect(within(group).getAllByRole("checkbox")).toHaveLength(names.length);
    for (const name of names)
      expect(within(group).getByRole("checkbox", { name: new RegExp(`^${name}`) })).toBeTruthy();
  }
  // 内容变了之后重新授权，也不预先勾上。
  expect(check("长期保存数据").checked).toBe(false);
  const warning = screen.getByText(PERSIST_DATA_WARNING);
  expect(warning.className).toContain("text-danger");
  expect(check("添加候选").closest("label")?.textContent).toContain("需要同时授予「读取候选词」");
  expect(screen.getByText("插件能读到你在详情页设置项里填的值，这不需要授权。")).toBeTruthy();
});

test("persist_data cannot be ticked while rewriting the commit is granted", () => {
  const pack = script({
    id: "both",
    permissions: ["read_commit", "rewrite_commit", "persist_data"],
  });
  render(
    <ScriptGrantDialog
      pack={pack}
      initial={["read_commit", "rewrite_commit"]}
      stale={false}
      onConfirm={vi.fn(async () => undefined)}
      onCancel={vi.fn()}
    />,
  );
  expect(check("长期保存数据").disabled).toBe(true);
  expect(check("长期保存数据").closest("label")?.textContent).toContain(
    "不能和「修改输入的文字」同时授予",
  );
});

test("extended commands add digits and operators to the provide_commands description", () => {
  const letters = script({
    id: "uuid",
    permissions: ["provide_commands"],
    commands: [{ trigger: "uuid", title: "UUID" }],
  });
  const extended = script({
    id: "calc",
    permissions: ["provide_commands"],
    commands: [{ trigger: "calc", title: "计算", arguments: "extended" }],
  });
  expect(capabilityDescription("provide_commands", letters)).toContain(
    "之后打的字母会交给这个插件",
  );
  const description = capabilityDescription("provide_commands", extended);
  expect(description).toContain("之后打的字母以及数字和运算符号会交给这个插件");
  expect(description).toContain("只取决于这些字符和随机数");
});

test("removing persist_data from a grant asks again before the stored data is deleted", async () => {
  const onConfirm = vi.fn(async (_granted: ScriptCapability[]) => undefined);
  render(
    <ScriptGrantDialog
      pack={memo}
      initial={persisting.plugins[0].granted}
      saved={persisting.plugins[0]}
      storageBytes={3000}
      stale={false}
      onConfirm={onConfirm}
      onCancel={vi.fn()}
    />,
  );
  expect(check("长期保存数据").checked).toBe(true);
  fireEvent.click(check("长期保存数据"));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(onConfirm).not.toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).toContain("插件已存的数据（3 KB）会被删除");
  // 改了勾选要重新确认。
  fireEvent.click(check("添加候选"));
  expect(screen.queryByRole("alert")).toBeNull();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "删除数据并授权" }));
  });
  expect(onConfirm).toHaveBeenCalledWith(["read_commit", "read_candidates"]);
});

test("the deletion rule follows the saved grant and what is on disk", () => {
  expect(grantDeletesStoredData(undefined, [], 0)).toBe(false);
  expect(grantDeletesStoredData(undefined, [], 10)).toBe(true);
  expect(grantDeletesStoredData({ granted: ["persist_data"] }, [], 0)).toBe(true);
  expect(grantDeletesStoredData({ granted: ["persist_data"] }, ["persist_data"], 10)).toBe(false);
  expect(formatKilobytes(0)).toBe("0 KB");
  expect(formatKilobytes(1)).toBe("1 KB");
  expect(formatKilobytes(5000)).toBe("5 KB");
});

test("the detail page saves the generated settings form and puts a refusal under its field", async () => {
  const setScriptSettings = vi
    .fn<NonNullable<PluginClient["setScriptSettings"]>>()
    .mockRejectedValueOnce({
      code: "plugin_script_setting_invalid",
      detail: "设置项 suffix 的值不合规",
    })
    .mockImplementation(async (_id, values) => pluginState({ values }));
  const client = fakeClient({ setScriptSettings });
  renderSection(client);
  await openMemo();
  const form = await screen.findByRole("region", { name: "设置" });
  expect(within(form).getByText(/插件能读到你在这里填的值/)).toBeTruthy();
  const save = within(form).getByRole<HTMLButtonElement>("button", { name: "保存设置" });
  expect(save.disabled).toBe(true);

  fireEvent.click(within(form).getByRole("switch", { name: "隐藏生僻字" }));
  fireEvent.change(within(form).getByRole("combobox", { name: "字表" }), {
    target: { value: "level_1" },
  });
  const threshold = within(form).getByRole<HTMLInputElement>("spinbutton", { name: "阈值" });
  expect(threshold.min).toBe("0");
  expect(threshold.max).toBe("9");
  fireEvent.change(threshold, { target: { value: "12" } });
  expect(within(form).getByRole("alert").textContent).toContain("请填 0 到 9 之间的整数");
  expect(save.disabled).toBe(true);
  fireEvent.change(threshold, { target: { value: "7" } });
  const suffix = within(form).getByRole<HTMLInputElement>("textbox", { name: "后缀" });
  expect(suffix.maxLength).toBe(4);
  fireEvent.change(suffix, { target: { value: "～" } });

  await act(async () => fireEvent.click(save));
  const values = { hide: false, level: "level_1", threshold: 7, suffix: "～" };
  expect(setScriptSettings).toHaveBeenLastCalledWith("memo", values);
  // 宿主说出了是哪一项：原因写在那一项下面。
  const refusal = within(form).getByRole("alert");
  expect(refusal.textContent).toContain("值不符合插件的要求");
  expect(refusal.closest("[data-row-title], div")?.textContent).toContain("后缀");

  fireEvent.change(suffix, { target: { value: "!" } });
  await act(async () => fireEvent.click(save));
  expect(setScriptSettings).toHaveBeenLastCalledWith("memo", { ...values, suffix: "!" });
  await waitFor(() => expect(within(form).getByText("已保存。")).toBeTruthy());
  expect(save.disabled).toBe(true);
});

test("bundled and stored data sizes show, and clearing asks first", async () => {
  const clearScriptData = vi.fn(async () => pluginState());
  const client = fakeClient({
    scriptGrants: vi.fn(async () => persisting),
    scriptPluginState: vi.fn(async () => pluginState({ storage_bytes: 2048 })),
    clearScriptData,
  });
  const confirm = vi.fn(async () => false);
  renderSection(client, confirm);
  await openMemo();
  const data = await screen.findByRole("region", { name: "数据" });
  expect(within(data).getByText("5 KB")).toBeTruthy();
  await waitFor(() => expect(within(data).getByText("2 KB")).toBeTruthy());

  const clear = within(data).getByRole("button", { name: "清除数据" });
  await act(async () => fireEvent.click(clear));
  expect(confirm).toHaveBeenCalledWith(
    expect.objectContaining({ title: "清除数据", danger: true }),
  );
  expect(clearScriptData).not.toHaveBeenCalled();

  confirm.mockResolvedValueOnce(true);
  await act(async () => fireEvent.click(clear));
  expect(clearScriptData).toHaveBeenCalledWith("memo");
  await waitFor(() => expect(within(data).getByText("0 KB")).toBeTruthy());
});

test("turning off a pack that keeps data confirms the deletion first", async () => {
  const setScriptGrant = vi.fn(async () => grants());
  const client = fakeClient({ scriptGrants: vi.fn(async () => persisting), setScriptGrant });
  const confirm = vi.fn(async () => false);
  renderSection(client, confirm);
  await openMemo();
  const toggle = screen.getByRole<HTMLInputElement>("switch", { name: "启用" });
  await waitFor(() => expect(toggle.checked).toBe(true));
  await act(async () => fireEvent.click(toggle));
  expect(confirm).toHaveBeenCalledWith(
    expect.objectContaining({ confirmLabel: "停用并删除数据", danger: true }),
  );
  expect(setScriptGrant).not.toHaveBeenCalled();
  confirm.mockResolvedValueOnce(true);
  await act(async () => fireEvent.click(toggle));
  expect(setScriptGrant).toHaveBeenCalledWith("memo", HASH, false, []);
});

test("the community page knows the new capabilities and flags keeping data", () => {
  render(
    <CommunityPluginCapabilities
      permissions={["read_candidates", "insert_candidates", "persist_data"]}
    />,
  );
  expect(screen.queryByText(/未知能力/)).toBeNull();
  expect(screen.queryByText(/还不认识的能力/)).toBeNull();
  expect(screen.getByText("添加候选")).toBeTruthy();
  const flagged = screen.getByText(/长期保存数据/);
  expect(flagged.className).toContain("text-danger");
  expect(flagged.textContent).toContain("注意：");
});
