// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  PluginsSection,
  ScriptGrantDialog,
  SettingsPage,
  defaultPluginPreferences,
  scriptGrantState,
  toggleCapability,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  type ScriptCapability,
  type ScriptGrantsResult,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const HASH = "0123456789abcdef".repeat(4);
const NEW_HASH = "fedcba9876543210".repeat(4);

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
    hooks: ["before_commit"],
    permissions: ["read_commit", "rewrite_commit"],
    memory_pages: 16,
    content_hash: HASH,
    ...overrides,
  }) as PluginPackage;

const pangu = script({ id: "pangu", name: "中英文之间加空格", author: "测试者" });
const counter = script({
  id: "counter",
  name: "上屏计数",
  hooks: ["commit"],
  permissions: [],
  content_hash: NEW_HASH,
});

const catalog: PluginCatalogResult = {
  packages: [
    { ...pangu },
    { ...counter },
    {
      id: "typewriter",
      kind: "sound",
      name: "打字机",
      version: "1.0.0",
      license: "CC0-1.0",
      author: null,
      description: null,
      builtin: false,
      mode: "keys",
    },
  ],
  issues: [{ kind: "script", folder: "broken", reason: "缺少 plugin.toml" }],
};

function grants(overrides: Partial<ScriptGrantsResult> = {}): ScriptGrantsResult {
  return { revision: 1, plugins: [], health: [], mac_required: false, signed: false, ...overrides };
}

function fakeClient(overrides: Partial<PluginClient> = {}): PluginClient {
  return {
    catalog: vi.fn(async () => catalog),
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => undefined),
    loadMentions: vi.fn(async () => []),
    saveMentions: vi.fn(async () => undefined),
    scriptGrants: vi.fn(async () => grants()),
    setScriptGrant: vi.fn(async () => grants()),
    reorderScripts: vi.fn(async () => grants()),
    ...overrides,
  };
}

function renderSection(client: PluginClient, platform = "macos", scriptPlugins = true) {
  const onError = vi.fn();
  render(
    <PluginsSection
      client={client}
      preferences={defaultPluginPreferences}
      keySound
      music
      triggers
      active
      scriptPlugins={scriptPlugins}
      platform={platform}
      onChange={vi.fn()}
      onError={onError}
      confirm={vi.fn(async () => true)}
    />,
  );
  return { onError };
}

async function openScript(name: string) {
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(await within(list).findByRole("button", { name }));
  await screen.findByRole("heading", { name });
}

function renderDialog(
  pack: PluginPackage,
  props: Partial<Parameters<typeof ScriptGrantDialog>[0]> = {},
) {
  const onConfirm = vi.fn(async (_granted: ScriptCapability[]) => undefined);
  const onCancel = vi.fn();
  render(
    <ScriptGrantDialog
      pack={pack}
      initial={[]}
      stale={false}
      unverified={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
      {...props}
    />,
  );
  return { onConfirm, onCancel };
}

const check = (name: string) => screen.getByRole("checkbox", { name: new RegExp(`^${name}`) });

test("granting a capability brings what it depends on, and revoking one takes its dependents", () => {
  const requested: ScriptCapability[] = [
    "read_commit",
    "rewrite_commit",
    "read_candidates",
    "rerank_candidates",
    "filter_candidates",
  ];
  expect(toggleCapability([], "rewrite_commit", true, requested)).toEqual([
    "read_commit",
    "rewrite_commit",
  ]);
  expect(
    toggleCapability(["read_commit", "rewrite_commit"], "read_commit", false, requested),
  ).toEqual([]);
  expect(
    toggleCapability(
      ["read_candidates", "rerank_candidates", "filter_candidates"],
      "read_candidates",
      false,
      requested,
    ),
  ).toEqual([]);
  expect(toggleCapability([], "filter_candidates", true, requested)).toEqual([
    "read_candidates",
    "filter_candidates",
  ]);
  // 只在清单申请过的范围里变动。
  expect(toggleCapability([], "annotate_candidates", true, requested)).toEqual([]);
});

test("rewriting the commit and reading the composition are never granted together", () => {
  const requested: ScriptCapability[] = ["read_composition", "read_commit", "rewrite_commit"];
  // 勾选改写时，读组字（和依赖它的）让开；反过来也一样，改写依赖的读上屏留着。
  expect(toggleCapability(["read_composition"], "rewrite_commit", true, requested)).toEqual([
    "read_commit",
    "rewrite_commit",
  ]);
  expect(
    toggleCapability(["read_commit", "rewrite_commit"], "read_composition", true, requested),
  ).toEqual(["read_composition", "read_commit"]);
});

test("the dialog offers the requested capabilities, links dependencies and disables the exclusive one", async () => {
  // 清单本身不能同时申请这两项；对话框照样守着这条规则。
  const pack = script({
    id: "both",
    permissions: ["read_composition", "read_commit", "rewrite_commit"],
  });
  const { onConfirm } = renderDialog(pack);
  expect(screen.getByRole("dialog", { name: "授权「both」" })).toBeTruthy();
  expect(screen.queryByRole("checkbox", { name: /^读取候选词/ })).toBeNull();

  fireEvent.click(check("修改输入的文字"));
  expect((check("修改输入的文字") as HTMLInputElement).checked).toBe(true);
  expect((check("读取输入的文字") as HTMLInputElement).checked).toBe(true);
  const composition = check("读取正在输入的内容") as HTMLInputElement;
  expect(composition.disabled).toBe(true);
  expect(composition.closest("label")?.textContent).toContain("不能和「修改输入的文字」同时授予");

  // 取消依赖项，依赖它的也一起取消，互斥项重新可选。
  fireEvent.click(check("读取输入的文字"));
  expect((check("修改输入的文字") as HTMLInputElement).checked).toBe(false);
  expect((check("读取正在输入的内容") as HTMLInputElement).disabled).toBe(false);
  fireEvent.click(check("读取正在输入的内容"));
  expect((check("修改输入的文字") as HTMLInputElement).disabled).toBe(true);

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(onConfirm).toHaveBeenCalledWith(["read_composition"]);
});

test("the dialog says a changed pack needs granting again and keeps open with the reason a refusal gives", async () => {
  const onConfirm = vi.fn(async () => {
    throw { code: "plugin_script_memory" };
  });
  renderDialog(pangu, { stale: true, onConfirm });
  expect(screen.getByText(/已更新，需要重新授权/)).toBeTruthy();
  // 新授权时什么都不预先勾选。
  expect((check("读取输入的文字") as HTMLInputElement).checked).toBe(false);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(onConfirm).toHaveBeenCalledWith([]);
  expect(screen.getByRole("alert").textContent).toContain("超出输入法留给脚本插件的内存");
  expect(screen.getByRole("dialog")).toBeTruthy();
});

test("the grant state follows the content hash, the signature and the switch", () => {
  const grant = { id: "pangu", hash: HASH, enabled: true, granted: [], verified: true };
  expect(scriptGrantState(pangu, undefined)).toBe("ungranted");
  expect(scriptGrantState(pangu, grant)).toBe("enabled");
  expect(scriptGrantState(pangu, { ...grant, enabled: false })).toBe("disabled");
  expect(scriptGrantState(pangu, { ...grant, hash: NEW_HASH })).toBe("stale");
  expect(scriptGrantState(pangu, { ...grant, verified: false })).toBe("unverified");
});

test("the script group is hidden unless the host runs script plugins", async () => {
  const client = fakeClient();
  renderSection(client, "macos", false);
  const list = await screen.findByLabelText("已安装的插件");
  await within(list).findByRole("button", { name: "打字机" });
  expect(screen.queryByText("脚本插件（实验）")).toBeNull();
  expect(within(list).queryByRole("button", { name: "中英文之间加空格" })).toBeNull();
  // 脚本文件夹的问题行也不列出。
  expect(screen.queryByText(/broken/)).toBeNull();
  expect(client.scriptGrants).not.toHaveBeenCalled();
});

test("the script group lists each pack with its grant state", async () => {
  const client = fakeClient({
    scriptGrants: vi.fn(async () =>
      grants({
        plugins: [
          { id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"], verified: true },
          { id: "counter", hash: HASH, enabled: false, granted: [], verified: true },
        ],
      }),
    ),
  });
  renderSection(client);
  expect(await screen.findByText("脚本插件（实验）")).toBeTruthy();
  const list = screen.getByLabelText("已安装的插件");
  const row = await within(list).findByRole("button", { name: "中英文之间加空格" });
  await waitFor(() => expect(row.textContent).toContain("已启用 · 第 1 位"));
  // counter 的授权是给旧内容的。
  expect(within(list).getByRole("button", { name: "上屏计数" }).textContent).toContain(
    "已更新，需要重新授权",
  );
  expect(screen.getByText(/broken/)).toBeTruthy();
});

test("enabling a pack goes through the grant dialog with the hash the page showed", async () => {
  const setScriptGrant = vi.fn(async () =>
    grants({
      plugins: [
        { id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"], verified: true },
      ],
    }),
  );
  const client = fakeClient({ setScriptGrant });
  renderSection(client);
  await openScript("中英文之间加空格");
  expect(screen.getByText("0123456789ab")).toBeTruthy();
  expect(screen.getByText("文字输入到应用之前（可以改写）")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  fireEvent.click(check("读取输入的文字"));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(setScriptGrant).toHaveBeenCalledWith("pangu", HASH, true, ["read_commit"]);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect((screen.getByRole("switch", { name: "启用" }) as HTMLInputElement).checked).toBe(true);
  expect(screen.getByText(/第 1 位，共 1 个/)).toBeTruthy();
});

test("a refusal over the memory ledger keeps the pack off and says why", async () => {
  const setScriptGrant = vi.fn(async () => {
    throw { code: "plugin_script_memory" };
  });
  renderSection(fakeClient({ setScriptGrant }));
  await openScript("中英文之间加空格");
  fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(screen.getByRole("alert").textContent).toContain("先停用别的脚本插件再试");
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect((screen.getByRole("switch", { name: "启用" }) as HTMLInputElement).checked).toBe(false);
});

test("Linux and Windows explain that other programs of the same user can forge grants; macOS does not", async () => {
  for (const platform of ["linux", "windows"]) {
    renderSection(fakeClient(), platform);
    await openScript("中英文之间加空格");
    expect(
      screen.getAllByText(/以你的身份运行的其他程序可以不经过这个页面/).length,
    ).toBeGreaterThan(0);
    cleanup();
  }
  renderSection(
    fakeClient({ scriptGrants: vi.fn(async () => grants({ mac_required: true, signed: true })) }),
  );
  await openScript("中英文之间加空格");
  expect(screen.queryByText(/以你的身份运行的其他程序/)).toBeNull();
  await waitFor(() => expect(screen.getByText(/其他程序伪造的授权不会生效/)).toBeTruthy());
});

test("without the keychain key a macOS page cannot enable a pack", async () => {
  renderSection(
    fakeClient({ scriptGrants: vi.fn(async () => grants({ mac_required: true, signed: false })) }),
  );
  await openScript("中英文之间加空格");
  await waitFor(() =>
    expect((screen.getByRole("switch", { name: "启用" }) as HTMLInputElement).disabled).toBe(true),
  );
  expect(screen.getByText(/读不到钥匙串里的授权密钥/)).toBeTruthy();
});

test("turning a pack off revokes its grant, and the order buttons reorder the running packs", async () => {
  const running = grants({
    plugins: [
      { id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"], verified: true },
      { id: "counter", hash: NEW_HASH, enabled: true, granted: [], verified: true },
    ],
  });
  const setScriptGrant = vi.fn(async () => grants());
  const reorderScripts = vi.fn(async () => running);
  renderSection(
    fakeClient({ scriptGrants: vi.fn(async () => running), setScriptGrant, reorderScripts }),
  );
  await openScript("上屏计数");
  await waitFor(() => expect(screen.getByText(/第 2 位，共 2 个/)).toBeTruthy());
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "提前" }));
  });
  expect(reorderScripts).toHaveBeenCalledWith(["counter", "pangu"]);
  await act(async () => {
    fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  });
  expect(setScriptGrant).toHaveBeenCalledWith("counter", NEW_HASH, false, []);
});

test("the settings page shows the script group only where the host declares script plugins", async () => {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
    },
  };
  const client = fakeClient();
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "linux", plugin_triggers: true, script_plugins: true }),
        plugins: client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  expect(await screen.findByText("脚本插件（实验）")).toBeTruthy();
  expect(client.scriptGrants).toHaveBeenCalled();
});
