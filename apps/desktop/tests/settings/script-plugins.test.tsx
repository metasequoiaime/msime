// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  PluginsSection,
  SCRIPT_BUILTIN_NOTE,
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
  return { revision: 1, plugins: [], health: [], commands: [], ...overrides };
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

function renderSection(
  client: PluginClient,
  platform = "macos",
  scriptPlugins = true,
  modes: Pick<Parameters<typeof PluginsSection>[0], "commandMode" | "onLocalMode"> = {},
) {
  const onError = vi.fn();
  const onChange = vi.fn();
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
      {...modes}
      onChange={onChange}
      onError={onError}
      confirm={vi.fn(async () => true)}
    />,
  );
  return { onError, onChange };
}

async function openScript(name: string) {
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(await within(list).findByRole("button", { name }));
  await screen.findByRole("heading", { name });
}

test("script detail shows shared pack metadata before its content fingerprint", async () => {
  renderSection(fakeClient());
  await openScript("中英文之间加空格");

  const info = screen.getByRole("region", { name: "信息" });
  expect(within(info).getByText("脚本插件（实验）")).toBeTruthy();
  expect(within(info).getByText("1.0.0")).toBeTruthy();
  expect(within(info).getByText("测试者")).toBeTruthy();
  expect(within(info).getByText("MIT")).toBeTruthy();
  expect(within(info).getByText(HASH.slice(0, 12))).toBeTruthy();
});

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

test("a capability the replaced pack no longer requests neither blocks its exclusive partner nor is granted", async () => {
  const before = script({
    id: "both",
    permissions: ["read_composition", "read_commit", "rewrite_commit"],
  });
  const onConfirm = vi.fn(async (_granted: ScriptCapability[]) => undefined);
  const dialog = (pack: PluginPackage) => (
    <ScriptGrantDialog
      pack={pack}
      initial={[]}
      stale={false}
      onConfirm={onConfirm}
      onCancel={vi.fn()}
    />
  );
  const { rerender } = render(dialog(before));
  fireEvent.click(check("修改输入的文字"));
  expect((check("读取正在输入的内容") as HTMLInputElement).disabled).toBe(true);

  // 页面重读之后 `pack` 换成不再申请「修改输入的文字」的新版本：它不再挡着互斥的那一项，也不交出去。
  rerender(dialog({ ...before, permissions: ["read_composition", "read_commit"] }));
  expect(screen.queryByRole("checkbox", { name: /^修改输入的文字/ })).toBeNull();
  expect((check("读取正在输入的内容") as HTMLInputElement).disabled).toBe(false);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(onConfirm).toHaveBeenCalledWith(["read_commit"]);
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

test("the grant state follows the content hash and the switch", () => {
  const grant = { id: "pangu", hash: HASH, enabled: true, granted: [] };
  expect(scriptGrantState(pangu, undefined)).toBe("ungranted");
  expect(scriptGrantState(pangu, grant)).toBe("enabled");
  expect(scriptGrantState(pangu, { ...grant, enabled: false })).toBe("disabled");
  expect(scriptGrantState(pangu, { ...grant, hash: NEW_HASH })).toBe("stale");
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
          { id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"] },
          { id: "counter", hash: HASH, enabled: false, granted: [] },
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
      plugins: [{ id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"] }],
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
  // 只有它一个在运行：顺序无从调整，「执行顺序」一组不出现。
  expect(screen.queryByText("执行顺序")).toBeNull();
  expect(screen.queryByRole("button", { name: "提前" })).toBeNull();
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

test("macOS, Linux and Windows all explain that other programs of the same user can forge grants", async () => {
  for (const platform of ["macos", "linux", "windows"]) {
    renderSection(fakeClient(), platform);
    await openScript("中英文之间加空格");
    expect(
      screen.getAllByText(/以你的身份运行的其他程序可以不经过这个页面/).length,
    ).toBeGreaterThan(0);
    cleanup();
  }
});

test("a macOS page never mentions the keychain and lets the user enable a pack", async () => {
  renderSection(fakeClient(), "macos");
  await openScript("中英文之间加空格");
  const toggle = screen.getByRole("switch", { name: "启用" }) as HTMLInputElement;
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(document.body.textContent).not.toMatch(/钥匙串|签名|无法核对/);
  fireEvent.click(toggle);
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.getByRole("dialog").textContent).toContain("在 macOS 上，以你的身份运行的其他程序");
});

test("turning a pack off revokes its grant, and the order buttons reorder the running packs", async () => {
  const running = grants({
    plugins: [
      { id: "pangu", hash: HASH, enabled: true, granted: ["read_commit"] },
      { id: "counter", hash: NEW_HASH, enabled: true, granted: [] },
    ],
  });
  const setScriptGrant = vi.fn(async () => grants());
  const reorderScripts = vi.fn(async () => running);
  renderSection(
    fakeClient({ scriptGrants: vi.fn(async () => running), setScriptGrant, reorderScripts }),
  );
  await openScript("上屏计数");
  await waitFor(() => expect(screen.getByText(/第 2 位，共 2 个/)).toBeTruthy());
  expect(screen.getByText("执行顺序")).toBeTruthy();
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
  fireEvent.click(screen.getByRole("button", { name: /^(插件|扩展插件)$/ }));
  expect(await screen.findByText("脚本插件（实验）")).toBeTruthy();
  expect(client.scriptGrants).toHaveBeenCalled();
});

test("a pack changed after the page read it is reread before the dialog explains", async () => {
  let current = catalog;
  const catalogCall = vi.fn(async () => current);
  const setScriptGrant = vi.fn(async () => {
    current = {
      ...catalog,
      packages: catalog.packages.map((pack) =>
        pack.id === "pangu" ? { ...pack, content_hash: NEW_HASH } : pack,
      ),
    };
    throw { code: "plugin_script_changed" };
  });
  renderSection(fakeClient({ catalog: catalogCall, setScriptGrant }));
  await openScript("中英文之间加空格");
  expect(screen.getByText("0123456789ab")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(screen.getByRole("alert").textContent).toContain("页面已经重新读取");
  expect(catalogCall).toHaveBeenCalledTimes(2);
  // 页面换成了现在的内容哈希，再确认就按它写入。
  expect(screen.getByText("fedcba987654")).toBeTruthy();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(setScriptGrant).toHaveBeenLastCalledWith("pangu", NEW_HASH, true, []);
});

test("after the pack changes under the dialog, only what the new version requests is granted", async () => {
  let current = catalog;
  const setScriptGrant = vi.fn(async () => {
    current = {
      ...catalog,
      packages: catalog.packages.map((pack) =>
        pack.id === "pangu"
          ? { ...pack, content_hash: NEW_HASH, permissions: ["read_commit" as const] }
          : pack,
      ),
    };
    throw { code: "plugin_script_changed" };
  });
  renderSection(fakeClient({ catalog: vi.fn(async () => current), setScriptGrant }));
  await openScript("中英文之间加空格");
  fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  fireEvent.click(check("修改输入的文字"));
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(setScriptGrant).toHaveBeenLastCalledWith("pangu", HASH, true, [
    "read_commit",
    "rewrite_commit",
  ]);
  // 新版本不再申请「修改输入的文字」：对话框不再列出它，再确认时也不把它交出去。
  expect(screen.queryByRole("checkbox", { name: /^修改输入的文字/ })).toBeNull();
  expect(check("读取输入的文字")).toHaveProperty("checked", true);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(setScriptGrant).toHaveBeenLastCalledWith("pangu", NEW_HASH, true, ["read_commit"]);
});

test("the grant dialog starts on cancel and gives focus back to the switch that opened it", async () => {
  renderSection(fakeClient());
  await openScript("中英文之间加空格");
  const toggle = screen.getByRole("switch", { name: "启用" });
  toggle.focus();
  fireEvent.click(toggle);
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "取消" })),
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(document.activeElement).toBe(screen.getByRole("switch", { name: "启用" }));
});

test("an IME stop is reported only while the pack is enabled", async () => {
  const health = [{ id: "pangu", hash: HASH, disabled: "failed" as const }];
  renderSection(
    fakeClient({
      scriptGrants: vi.fn(async () =>
        grants({
          health,
          plugins: [{ id: "pangu", hash: HASH, enabled: false, granted: [] }],
        }),
      ),
    }),
  );
  await openScript("中英文之间加空格");
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("已停用"));
  expect(screen.getByRole("status").textContent).not.toContain("输入法已停用它");
});

const uuidPack = script({
  id: "uuid",
  name: "UUID 样板",
  hooks: ["command"],
  permissions: ["provide_commands"],
  commands: [
    { trigger: "uuid", title: "UUID" },
    { trigger: "guid", title: "GUID" },
  ],
});

test("a pack that provides slash commands can be granted without any dependency or exclusion", async () => {
  // 没有依赖、不和任何一项互斥：和读写文字的能力一起勾也不互相影响。
  expect(
    toggleCapability(["read_commit"], "provide_commands", true, [
      "read_commit",
      "provide_commands",
    ]),
  ).toEqual(["read_commit", "provide_commands"]);
  expect(
    toggleCapability(["read_commit", "provide_commands"], "read_commit", false, [
      "read_commit",
      "provide_commands",
    ]),
  ).toEqual(["provide_commands"]);

  const { onConfirm } = renderDialog(uuidPack);
  const box = check("提供 / 指令") as HTMLInputElement;
  expect(box.disabled).toBe(false);
  const label = box.closest("label")?.textContent ?? "";
  expect(label).toContain("给出的候选只取决于这些字母和随机数");
  expect(label).toContain(
    "按回车时只输入你打的原文（例如 /uuid），点到别的窗口时插件给出的行不会输入",
  );
  expect(label).toContain("按快捷键或 Tab 等键、用菜单或工具栏结束输入时也会输入这一行");
  expect(label).not.toContain("需要同时授予");
  expect(label).not.toContain("不能和");
  fireEvent.click(box);
  expect(box.checked).toBe(true);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "授权并启用" }));
  });
  expect(onConfirm).toHaveBeenCalledWith(["provide_commands"]);
});

test("the detail of a pack that provides slash commands names its capability and when it runs", async () => {
  renderSection(fakeClient({ catalog: vi.fn(async () => ({ packages: [uuidPack], issues: [] })) }));
  await openScript("UUID 样板");
  expect(screen.getByText("你在 / 指令里输入它的触发词时（给出候选）")).toBeTruthy();
  const requested = screen.getByRole("region", { name: "申请的权限" });
  expect(within(requested).getByText("提供 / 指令")).toBeTruthy();
  expect(within(requested).getByText("未授予")).toBeTruthy();
});

/** `uuid` 启用并授予了「提供 / 指令」的授权，`commands` 是宿主算出的状态。 */
function uuidGrants(commands: ScriptGrantsResult["commands"]): ScriptGrantsResult {
  return grants({
    plugins: [{ id: "uuid", hash: HASH, enabled: true, granted: ["provide_commands"] }],
    commands,
  });
}

const otherUuid = script({
  id: "other",
  name: "另一个 UUID",
  hooks: ["command"],
  permissions: ["provide_commands"],
  commands: [{ trigger: "uu", title: "UU" }],
});

test("the detail lists each slash command with its state and names who shadows it", async () => {
  renderSection(
    fakeClient({
      catalog: vi.fn(async () => ({ packages: [uuidPack, otherUuid], issues: [] })),
      scriptGrants: vi.fn(async () =>
        uuidGrants([
          { id: "other", trigger: "uu", title: "UU", status: "active" },
          {
            id: "uuid",
            trigger: "uuid",
            title: "UUID",
            status: "shadowed_by_plugin",
            conflict_id: "other",
            conflict: "uu",
          },
          {
            id: "uuid",
            trigger: "guid",
            title: "GUID",
            status: "shadowed_by_user_table",
            conflict: "gu",
          },
        ]),
      ),
    }),
    "macos",
    true,
    { commandMode: true, onLocalMode: vi.fn() },
  );
  await openScript("UUID 样板");
  const commands = await screen.findByRole("region", { name: "提供的 / 指令" });
  await waitFor(() =>
    expect(within(commands).getByText("与插件 另一个 UUID 的 /uu 冲突，未生效")).toBeTruthy(),
  );
  expect(within(commands).getByText("/uuid")).toBeTruthy();
  expect(within(commands).getByText("UUID")).toBeTruthy();
  expect(within(commands).getByText("/guid")).toBeTruthy();
  expect(within(commands).getByText("与你的指令 /gu 冲突，未生效")).toBeTruthy();
  // / 指令开着：没有提示，也没有按钮。
  expect(within(commands).queryByRole("button", { name: "打开 / 指令" })).toBeNull();
});

test("an active slash command says so, and a command of a pack not yet granted says how it takes effect", async () => {
  renderSection(
    fakeClient({
      catalog: vi.fn(async () => ({ packages: [uuidPack], issues: [] })),
      scriptGrants: vi.fn(async () =>
        uuidGrants([{ id: "uuid", trigger: "uuid", title: "UUID", status: "active" }]),
      ),
    }),
    "macos",
    true,
    { commandMode: true },
  );
  await openScript("UUID 样板");
  const commands = await screen.findByRole("region", { name: "提供的 / 指令" });
  await waitFor(() => expect(within(commands).getByText("生效")).toBeTruthy());
  // 宿主没列出 `guid`（例如插件超过 8 个、包载入不了）：照实说没生效。
  expect(within(commands).getByText("未生效")).toBeTruthy();
  cleanup();

  renderSection(fakeClient({ catalog: vi.fn(async () => ({ packages: [uuidPack], issues: [] })) }));
  await openScript("UUID 样板");
  const pending = await screen.findByRole("region", { name: "提供的 / 指令" });
  expect(within(pending).getAllByText("授权并启用后生效")).toHaveLength(2);
});

test("with the slash mode off the detail explains it and only the button turns it on", async () => {
  const onLocalMode = vi.fn();
  const setScriptGrant = vi.fn(async () =>
    uuidGrants([{ id: "uuid", trigger: "uuid", title: "UUID", status: "active" }]),
  );
  const { onChange } = renderSection(
    fakeClient({
      catalog: vi.fn(async () => ({ packages: [uuidPack], issues: [] })),
      setScriptGrant,
    }),
    "macos",
    true,
    { commandMode: false, onLocalMode },
  );
  await openScript("UUID 样板");
  const commands = await screen.findByRole("region", { name: "提供的 / 指令" });
  expect(
    within(commands).getByText(
      "需要先打开 / 指令，并使用中文标点；在全拼、双拼、五笔下没有输入时按 / 再输入触发词。",
    ),
  ).toBeTruthy();
  // 授权不碰偏好：经对话框授权并启用之后，/ 指令仍然是关的。
  fireEvent.click(screen.getByRole("switch", { name: "启用" }));
  const dialog = await screen.findByRole("dialog");
  const listed = within(dialog).getByRole("list", { name: "它提供的 / 指令" });
  expect(within(listed).getByText("/uuid UUID")).toBeTruthy();
  expect(within(listed).getByText("/guid GUID")).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("checkbox", { name: /提供 \/ 指令/ }));
  await act(async () => {
    fireEvent.click(within(dialog).getByRole("button", { name: "授权并启用" }));
  });
  expect(setScriptGrant).toHaveBeenCalledWith("uuid", HASH, true, ["provide_commands"]);
  expect(onLocalMode).not.toHaveBeenCalled();
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.click(within(commands).getByRole("button", { name: "打开 / 指令" }));
  expect(onLocalMode).toHaveBeenCalledWith("command");
});

test("an official script pack says it ships with the input method, cannot be removed and runs only once granted", async () => {
  const official = script({
    id: "uuid",
    name: "UUID",
    builtin: true,
    hooks: ["command"],
    permissions: ["provide_commands"],
    commands: [{ trigger: "uuid", title: "UUID" }],
    content_hash: NEW_HASH,
  } as Partial<PluginPackage> & Pick<PluginPackage, "id">);
  const client = fakeClient({
    catalog: vi.fn(async () => ({ packages: [official], issues: [] })),
    // 授权给的是上一个版本的内容：内置插件只会随输入法更新而变。
    scriptGrants: vi.fn(async () =>
      grants({
        plugins: [{ id: "uuid", hash: HASH, enabled: true, granted: ["provide_commands"] }],
      }),
    ),
  });
  renderSection(client);
  const list = await screen.findByLabelText("已安装的插件");
  const row = await within(list).findByRole("button", { name: /UUID/ });
  await waitFor(() => expect(row.textContent).toContain("已随输入法更新，需要重新授权"));
  fireEvent.click(row);
  await screen.findByRole("heading", { name: "UUID" });
  const info = screen.getByRole("region", { name: "信息" });
  expect(within(info).getByText(SCRIPT_BUILTIN_NOTE)).toBeTruthy();
  expect(screen.getByText(/已随输入法更新，需要重新授权：插件的内容和授权时不同/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "删除UUID" })).toBeNull();
  expect(client.remove).not.toHaveBeenCalled();
});

test("an imported script pack keeps its delete button and the plain update wording", async () => {
  renderSection(
    fakeClient({
      scriptGrants: vi.fn(async () =>
        grants({ plugins: [{ id: "pangu", hash: NEW_HASH, enabled: true, granted: [] }] }),
      ),
    }),
  );
  await openScript("中英文之间加空格");
  expect(screen.queryByText(SCRIPT_BUILTIN_NOTE)).toBeNull();
  expect(screen.getByRole("button", { name: "删除中英文之间加空格" })).toBeTruthy();
  expect(screen.getByText(/^已更新，需要重新授权：插件的内容和授权时不同/)).toBeTruthy();
});
