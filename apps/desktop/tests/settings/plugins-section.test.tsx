// @vitest-environment jsdom
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  LocalModesSection,
  PluginsSection,
  SettingsPage,
  defaultLocalModes,
  defaultPluginPreferences,
  mentionListIssue,
  pluginErrorMessage,
  pluginPreferences,
  settingsCapabilities,
  withPackSelected,
  withoutRemovedPack,
  type MentionEntry,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  type PluginPreferences,
  type Preferences,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("ignores a duplicate plugin import while the first import is pending", async () => {
  let resolveImport!: (value: null) => void;
  const importPack = vi.fn(() => new Promise<null>((resolve) => (resolveImport = resolve)));
  const client = fakeClient({ importPack });
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "linux", plugin_triggers: true } as never,
        plugins: client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  const importButton = await screen.findByRole("button", { name: "导入文件夹" });
  await act(async () => {
    fireEvent.click(importButton);
    fireEvent.click(importButton);
  });
  expect(importPack).toHaveBeenCalledOnce();
  resolveImport(null);
  await waitFor(() => expect(importPack).toHaveBeenCalledOnce());
});

test("a plugin removal response from a replaced client cannot update preferences", async () => {
  let resolveRemove!: () => void;
  const oldClient = fakeClient({
    remove: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveRemove = resolve;
        }),
    ),
  });
  const nextClient = fakeClient();
  const onChange = vi.fn();
  const view = render(
    <PluginsSection
      client={oldClient}
      preferences={defaultPluginPreferences}
      keySound
      music
      triggers
      active
      onChange={onChange}
      onError={vi.fn()}
      confirm={vi.fn(async () => true)}
    />,
  );
  await openPack("打字机");
  fireEvent.click(screen.getByRole("button", { name: "删除打字机" }));
  await waitFor(() => expect(oldClient.remove).toHaveBeenCalledWith("sound", "typewriter"));

  view.rerender(
    <PluginsSection
      client={nextClient}
      preferences={defaultPluginPreferences}
      keySound
      music
      triggers
      active
      onChange={onChange}
      onError={vi.fn()}
      confirm={vi.fn(async () => true)}
    />,
  );
  await waitFor(() => expect(nextClient.catalog).toHaveBeenCalled());
  await act(async () => {
    resolveRemove();
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(onChange).not.toHaveBeenCalled();
  // Nor close the detail it was started from.
  expect(screen.getByRole("heading", { name: "打字机" })).toBeTruthy();
});

const pack = (overrides: Partial<PluginPackage> & Pick<PluginPackage, "id" | "kind">) =>
  ({
    name: overrides.id,
    version: "1.0.0",
    license: "CC0-1.0",
    author: null,
    description: null,
    builtin: false,
    ...overrides,
  }) as PluginPackage;

const catalog: PluginCatalogResult = {
  packages: [
    pack({ id: "default", kind: "sound", name: "默认", builtin: true, mode: "keys" }),
    pack({ id: "twinkle", kind: "sound", name: "小星星", builtin: true, mode: "sequence" }),
    pack({ id: "typewriter", kind: "sound", name: "打字机", author: "测试者", mode: "keys" }),
    pack({ id: "rain", kind: "music", name: "雨声", license: "CC-BY-4.0", tracks: ["a.ogg"] }),
    pack({
      id: "signature",
      kind: "command_table",
      name: "签名",
      commands: [{ trigger: "sig", title: "签名", template: "{date} 测试" }],
    }),
  ],
  issues: [{ kind: "sound", folder: "broken", reason: "缺少 plugin.toml" }],
};

function fakeClient(overrides: Partial<PluginClient> = {}): PluginClient {
  return {
    catalog: vi.fn(async () => catalog),
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => undefined),
    loadMentions: vi.fn(async (): Promise<MentionEntry[]> => [{ text: "张三", key: "zhang'san" }]),
    saveMentions: vi.fn(async () => undefined),
    ...overrides,
  };
}

function renderSection(
  props: Partial<Parameters<typeof PluginsSection>[0]> = {},
  preferences: PluginPreferences = defaultPluginPreferences,
) {
  const onChange = vi.fn();
  const onError = vi.fn();
  const confirm = vi.fn(async () => true);
  const client = props.client === undefined && !("client" in props) ? fakeClient() : props.client;
  render(
    <PluginsSection
      preferences={preferences}
      keySound
      music
      triggers
      active
      onChange={onChange}
      onError={onError}
      confirm={confirm}
      {...props}
      client={client}
    />,
  );
  return { onChange, onError, confirm, client };
}

/** Opens the 声音与效果 view from the 我的插件 list. */
function openSoundEffects() {
  fireEvent.click(screen.getByRole("button", { name: "声音与效果" }));
}

/** Opens an installed pack's detail from the list, once the catalog has listed it. */
async function openPack(name: string) {
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(await within(list).findByRole("button", { name }));
  await screen.findByRole("heading", { name });
}

function openMentions() {
  fireEvent.click(screen.getByRole("button", { name: "@ 名单" }));
}

function backToList() {
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
}

test("switches key sounds and keeps the other effect settings", () => {
  const { onChange } = renderSection({ client: undefined });
  openSoundEffects();

  fireEvent.click(screen.getByRole("switch", { name: "按键音" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, enabled: true },
  });

  fireEvent.click(screen.getByRole("switch", { name: "上屏音" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    commit_sound: { enabled: true },
  });

  fireEvent.click(screen.getByRole("switch", { name: "成就音效" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    achievements: { enabled: true },
  });

  fireEvent.change(screen.getByRole("slider", { name: "音效音量" }), { target: { value: "80" } });
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, volume: 80 },
  });
});

test("offers the melody only in melody mode, and the mode only while key sounds are on", async () => {
  const on = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, enabled: true },
  };
  const { onChange } = renderSection({}, on);
  await screen.findByRole("button", { name: "打字机" });
  openSoundEffects();

  // The 旋律 row is kept but hidden while the mode plays the key classes.
  expect(screen.getByText("小星星（内置）").closest("[hidden]")).not.toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "按键旋律" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...on,
    key_sound: { ...on.key_sound, mode: "melody" },
  });

  cleanup();
  renderSection({}, { ...on, key_sound: { ...on.key_sound, mode: "melody" } });
  await screen.findByRole("button", { name: "打字机" });
  openSoundEffects();
  expect(screen.getByText("小星星（内置）").closest("[hidden]")).toBeNull();
  expect(screen.getByText("默认（内置）")).toBeTruthy();

  cleanup();
  renderSection({ client: undefined });
  openSoundEffects();
  expect(
    screen.getByRole("radiogroup", { name: "发声方式" }).querySelector("input")?.disabled,
  ).toBe(true);
});

test("a melody pack is chosen on its own detail, which says where the mode is switched", async () => {
  const melodies = fakeClient({
    catalog: vi.fn(async () => ({
      packages: [
        ...catalog.packages,
        pack({ id: "canon", kind: "sound", name: "卡农", mode: "sequence" }),
      ],
      issues: [],
    })),
  });
  const { onChange } = renderSection({ client: melodies });
  await openPack("卡农");
  expect(screen.getByText(/把发声方式设为按键旋律后/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "设为按键旋律" }));
  // The mode is left as it was: choosing a melody does not start playing it.
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    melody: { pack: "canon" },
  });

  backToList();
  await openPack("小星星");
  const current = screen.getByRole("button", { name: "使用中" }) as HTMLButtonElement;
  expect(current.disabled).toBe(true);
});

test("lists a selected pack that is no longer installed, and drops it from its own view", async () => {
  const gone = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "gone" },
  };
  const { onChange } = renderSection({}, gone);
  const list = await screen.findByLabelText("已安装的插件");
  const row = await within(list).findByRole("button", { name: "gone（未找到）" });
  expect(row.textContent).toContain("当前音效包，已不在本机");

  openSoundEffects();
  expect(screen.getByText("gone（未找到）")).toBeTruthy();
  backToList();

  fireEvent.click(await screen.findByRole("button", { name: "gone（未找到）" }));
  expect(screen.getByRole("heading", { name: "gone（未找到）" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "改回默认" }));
  expect(onChange).toHaveBeenLastCalledWith(defaultPluginPreferences);
  expect(await screen.findByLabelText("已安装的插件")).toBeTruthy();

  cleanup();
  // A host with no pack store lists nothing, so the selection is shown as it is rather than as missing.
  renderSection({ client: undefined });
  openSoundEffects();
  expect(screen.getByText("default")).toBeTruthy();
  expect(screen.queryByText("default（未找到）")).toBeNull();
});

test("a sound pack selected in the other mode is listed as unusable and reset on its own", async () => {
  const crossed = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "twinkle" },
  };
  const { onChange } = renderSection({}, crossed);
  const list = await screen.findByLabelText("已安装的插件");
  const row = await within(list).findByRole("button", { name: "twinkle（不可用）" });
  expect(row.textContent).toContain("当前音效包，但它是按键旋律，不能用作按键音效");
  const sounds = within(list).getByRole("region", { name: "音效包" });
  expect(sounds.textContent).not.toContain("使用中");

  openSoundEffects();
  expect(screen.getByText("twinkle（不可用）")).toBeTruthy();
  backToList();

  fireEvent.click(await screen.findByRole("button", { name: "twinkle（不可用）" }));
  fireEvent.click(screen.getByRole("button", { name: "改回默认" }));
  // The melody keeps the pack, which is rightly selected there.
  expect(onChange).toHaveBeenLastCalledWith(defaultPluginPreferences);

  cleanup();
  const melodyCrossed = { ...defaultPluginPreferences, melody: { pack: "typewriter" } };
  renderSection({}, melodyCrossed);
  const melodyRow = await screen.findByRole("button", { name: "typewriter（不可用）" });
  expect(melodyRow.textContent).toContain("当前旋律，但它不是按键旋律");
});

test("the list says the catalog is being read, or could not be, rather than that nothing is installed", async () => {
  renderSection({
    client: fakeClient({ catalog: vi.fn(() => new Promise<PluginCatalogResult>(() => {})) }),
  });
  expect(screen.getByText("正在读取插件…")).toBeTruthy();
  expect(screen.queryByText("没有插件")).toBeNull();
  expect(screen.queryByText("还没有导入指令表。")).toBeNull();

  cleanup();
  const { onError } = renderSection({
    client: fakeClient({ catalog: vi.fn(async () => Promise.reject(new Error("offline"))) }),
  });
  expect(await screen.findByText(/插件列表没有读取成功/)).toBeTruthy();
  expect(onError).toHaveBeenCalled();
  expect(screen.queryByText("没有插件")).toBeNull();
  expect(screen.queryByText("还没有导入指令表。")).toBeNull();
  expect(screen.queryByText("正在读取插件…")).toBeNull();
});

test("without a pack store, 声音与效果 does not point at packs the list cannot show", () => {
  renderSection({ client: undefined, typingEffects: true, effectStyles: true, effectPacks: true });
  openSoundEffects();
  expect(screen.queryByText(/在「我的插件」里打开/)).toBeNull();
  expect(screen.getAllByText(/这台设备只能使用内置的包。/).length).toBeGreaterThan(0);
});

test("a pack of a kind the host does not act on is listed but not marked in use", async () => {
  const selected = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "typewriter" },
    music: { ...defaultPluginPreferences.music, pack: "rain" },
  };
  renderSection({ keySound: false, music: false }, selected);
  const list = await screen.findByLabelText("已安装的插件");
  const row = await within(list).findByRole("button", { name: "打字机" });
  expect(row.textContent).not.toContain("使用中");
  expect(within(list).getByRole("button", { name: "小星星" }).textContent).not.toContain(
    "当前旋律",
  );
  expect(within(list).getByRole("button", { name: "雨声" }).textContent).not.toContain("使用中");
});

test("after deleting a pack or dropping a missing selection, focus lands on the list, not the page body", async () => {
  let removed = false;
  renderSection({
    client: fakeClient({
      catalog: vi.fn(async () =>
        removed
          ? { ...catalog, packages: catalog.packages.filter((item) => item.id !== "typewriter") }
          : catalog,
      ),
      remove: vi.fn(async () => {
        removed = true;
      }),
    }),
  });
  await openPack("打字机");
  fireEvent.click(screen.getByRole("button", { name: "删除打字机" }));
  const list = await screen.findByLabelText("已安装的插件");
  const sounds = within(list).getByRole("region", { name: "音效包" });
  await waitFor(() =>
    expect(document.activeElement).toBe(within(sounds).getAllByRole("button")[0]),
  );

  cleanup();
  const gone = {
    ...defaultPluginPreferences,
    music: { ...defaultPluginPreferences.music, pack: "gone" },
  };
  const client = fakeClient();
  // The page holds the preferences, so dropping the selection and closing the view land in one render.
  function Page() {
    const [preferences, setPreferences] = useState<PluginPreferences>(gone);
    return (
      <PluginsSection
        client={client}
        preferences={preferences}
        keySound
        music
        triggers
        active
        onChange={setPreferences}
        onError={vi.fn()}
        confirm={vi.fn(async () => true)}
      />
    );
  }
  render(<Page />);
  fireEvent.click(await screen.findByRole("button", { name: "gone（未找到）" }));
  fireEvent.click(screen.getByRole("button", { name: "不再使用" }));
  expect(screen.queryByRole("button", { name: "gone（未找到）" })).toBeNull();
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "雨声" })),
  );
});

test("chooses a music pack on its detail, which starts unselected", async () => {
  const { onChange } = renderSection();
  await screen.findByRole("button", { name: "雨声" });
  openSoundEffects();
  expect(screen.getByText("未选择")).toBeTruthy();
  backToList();

  await openPack("雨声");
  expect(screen.getByText("a.ogg")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "设为当前音乐包" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    music: { ...defaultPluginPreferences.music, pack: "rain" },
  });

  cleanup();
  const playing = {
    ...defaultPluginPreferences,
    music: { enabled: true, pack: "rain", volume: 30 },
  };
  const next = renderSection({}, playing);
  await openPack("雨声");
  expect((screen.getByRole("button", { name: "使用中" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "不再使用" }));
  expect(next.onChange).toHaveBeenLastCalledWith({
    ...playing,
    music: { ...playing.music, pack: "" },
  });
});

test("says built-in commands still work where command tables cannot be imported", () => {
  renderSection({ client: undefined });
  expect(
    screen.getByText("这台设备还不能导入指令表，内置的 rq、sj、xq 指令照常可用。"),
  ).toBeTruthy();
  expect(screen.queryByText("还没有导入指令表。")).toBeNull();
  expect(screen.queryByText("@ 名单")).toBeNull();
  expect(screen.queryByRole("button", { name: "导入文件夹" })).toBeNull();
  // The settings that belong to no pack are still reachable without a pack store.
  openSoundEffects();
  expect(screen.getByRole("switch", { name: "按键音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "背景音乐" })).toBeTruthy();
  backToList();
  expect(screen.getByRole("button", { name: "声音与效果" })).toBeTruthy();
});

test("does not promise a password-field pause the Windows host cannot make", () => {
  renderSection();
  openSoundEffects();
  expect(
    screen.getByText("默认关闭。只在输入法处于活动状态时播放，切换到其他输入法时暂停。"),
  ).toBeTruthy();
});

test("hides the groups a host does not back", () => {
  renderSection({ client: undefined, keySound: false, music: false, triggers: false });
  expect(screen.queryByRole("button", { name: "声音与效果" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "按键音" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "背景音乐" })).toBeNull();
  expect(screen.queryByText("指令表")).toBeNull();
  expect(screen.queryByText("@ 名单")).toBeNull();
  expect(screen.queryByRole("button", { name: "导入文件夹" })).toBeNull();
});

test("lists the installed packs by kind with what each is used as, and removes only installed ones", async () => {
  const selected = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "typewriter" },
  };
  const { client, confirm, onChange } = renderSection({}, selected);
  const list = await screen.findByLabelText("已安装的插件");
  await within(list).findByRole("button", { name: "打字机" });

  expect(
    within(list)
      .getAllByRole("region")
      .map((group) => group.querySelector("h3")?.textContent),
  ).toEqual(["音效包", "音乐包", "指令表"]);
  const sounds = within(list).getByRole("region", { name: "音效包" });
  const rows = within(sounds).getAllByRole("button");
  expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual(["默认", "小星星", "打字机"]);
  expect(rows.every((row) => row.getAttribute("type") === "button")).toBe(true);
  expect(rows[0].textContent).toBe("默认1.0.0 · 内置›");
  expect(rows[1].textContent).toBe("小星星1.0.0 · 按键旋律 · 内置当前旋律›");
  expect(rows[2].textContent).toBe("打字机1.0.0 · 作者 测试者使用中›");
  expect(within(list).getByRole("button", { name: "雨声" }).textContent).not.toContain("使用中");
  expect(screen.getByText("音效包 broken 无法载入：缺少 plugin.toml")).toBeTruthy();

  await openPack("默认");
  expect(screen.getByText("内置，不能删除")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "删除默认" })).toBeNull();
  backToList();

  await openPack("打字机");
  expect(screen.getByText("CC0-1.0")).toBeTruthy();
  expect(screen.getByText("测试者")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "删除打字机" }));
  await waitFor(() => expect(client!.remove).toHaveBeenCalledWith("sound", "typewriter"));
  expect(confirm).toHaveBeenCalledWith(expect.objectContaining({ danger: true }));
  // The selection falls back to the built-in pack rather than naming one that is gone.
  expect(onChange).toHaveBeenCalledWith(defaultPluginPreferences);
  // The detail of a deleted pack closes back to the list.
  expect(await screen.findByLabelText("已安装的插件")).toBeTruthy();
  expect(screen.queryByRole("heading", { name: "打字机" })).toBeNull();
});

test("a row opens its pack's detail and the back button returns focus to it", async () => {
  renderSection();
  const row = await within(await screen.findByLabelText("已安装的插件")).findByRole("button", {
    name: "签名",
  });
  expect(row.tagName).toBe("BUTTON");
  row.focus();
  fireEvent.click(row);
  const back = screen.getByRole("button", { name: "返回我的插件" });
  expect(document.activeElement).toBe(back);
  expect(screen.getByRole("heading", { name: "签名" })).toBeTruthy();
  expect(screen.queryByLabelText("已安装的插件")).toBeNull();

  fireEvent.click(back);
  const again = await within(screen.getByLabelText("已安装的插件")).findByRole("button", {
    name: "签名",
  });
  expect(document.activeElement).toBe(again);
});

test("leaving the page closes an open view, so the next visit starts at the list", async () => {
  const client = fakeClient();
  const props = {
    client,
    preferences: defaultPluginPreferences,
    keySound: true,
    music: true,
    triggers: true,
    onChange: vi.fn(),
    onError: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<PluginsSection {...props} active />);
  await openPack("打字机");
  view.rerender(<PluginsSection {...props} active={false} />);
  view.rerender(<PluginsSection {...props} active />);
  expect(screen.queryByRole("heading", { name: "打字机" })).toBeNull();
  expect(await screen.findByLabelText("已安装的插件")).toBeTruthy();

  openSoundEffects();
  view.rerender(<PluginsSection {...props} active={false} />);
  view.rerender(<PluginsSection {...props} active />);
  expect(screen.queryByRole("switch", { name: "按键音" })).toBeNull();
});

test("an import left behind when the page was closed releases the buttons once it settles", async () => {
  let resolveImport!: (value: null) => void;
  const client = fakeClient({
    importPack: vi.fn(() => new Promise<null>((resolve) => (resolveImport = resolve))),
  });
  const props = {
    client,
    preferences: defaultPluginPreferences,
    keySound: true,
    music: true,
    triggers: true,
    onChange: vi.fn(),
    onError: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<PluginsSection {...props} active />);
  await screen.findByRole("button", { name: "打字机" });
  fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "导入文件夹" }) as HTMLButtonElement).disabled).toBe(
      true,
    ),
  );
  view.rerender(<PluginsSection {...props} active={false} />);
  await act(async () => {
    resolveImport(null);
    await Promise.resolve();
    await Promise.resolve();
  });
  view.rerender(<PluginsSection {...props} active />);
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "导入文件夹" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
});

test("keeps a pack when the removal is not confirmed", async () => {
  const client = fakeClient();
  render(
    <PluginsSection
      client={client}
      preferences={defaultPluginPreferences}
      keySound
      music
      triggers
      active
      onChange={vi.fn()}
      onError={vi.fn()}
      confirm={async () => false}
    />,
  );
  await openPack("打字机");
  fireEvent.click(screen.getByRole("button", { name: "删除打字机" }));
  await waitFor(() => expect(client.remove).not.toHaveBeenCalled());
  expect(screen.getByRole("heading", { name: "打字机" })).toBeTruthy();
});

test("imports through the host picker and reports what was installed", async () => {
  const imported = pack({ id: "piano", kind: "sound", name: "钢琴", version: "2.0.0" });
  const client = fakeClient({ importPack: vi.fn(async () => imported) });
  renderSection({ client });
  await screen.findByRole("button", { name: "打字机" });

  fireEvent.click(screen.getByRole("button", { name: "导入 .zip" }));
  await waitFor(() => expect(client.importPack).toHaveBeenCalledWith("archive"));
  expect(await screen.findByText("已导入音效包「钢琴」2.0.0。")).toBeTruthy();
  expect(client.catalog).toHaveBeenCalledTimes(2);

  fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));
  await waitFor(() => expect(client.importPack).toHaveBeenLastCalledWith("folder"));
});

test("says which rule a refused pack broke", async () => {
  const client = fakeClient({
    importPack: vi.fn(async () => {
      throw { code: "plugin_invalid", detail: "插件不能申请权限，permissions 必须为空" };
    }),
  });
  const { onError } = renderSection({ client });
  await screen.findByRole("button", { name: "打字机" });

  fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));
  await waitFor(() =>
    expect(onError).toHaveBeenCalledWith(
      "插件不符合要求（插件不能申请权限，permissions 必须为空）。",
    ),
  );
});

test("a cancelled picker changes nothing", async () => {
  const client = fakeClient();
  renderSection({ client });
  await screen.findByRole("button", { name: "打字机" });
  fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));
  await waitFor(() => expect(client.importPack).toHaveBeenCalled());
  expect(client.catalog).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("status")).toBeNull();
});

test("enables a command table on its detail and drops an enabled table that is gone", async () => {
  const { onChange } = renderSection(
    {},
    { ...defaultPluginPreferences, command_tables: ["removed"] },
  );
  const list = await screen.findByLabelText("已安装的插件");
  const tables = within(list).getByRole("region", { name: "指令表" });
  await within(tables).findByRole("button", { name: "removed（未找到）" });

  await openPack("签名");
  expect(screen.getByText("/sig")).toBeTruthy();
  const table = screen.getByRole("switch", { name: "启用" });
  fireEvent.click(table);
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    command_tables: ["removed", "signature"],
  });
  backToList();

  fireEvent.click(await screen.findByRole("button", { name: "removed（未找到）" }));
  fireEvent.click(screen.getByRole("button", { name: "移除" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...defaultPluginPreferences, command_tables: [] });
});

test("marks an enabled command table with its place", async () => {
  renderSection({}, { ...defaultPluginPreferences, command_tables: ["signature"] });
  const row = await screen.findByRole("button", { name: "签名" });
  expect(row.textContent).toContain("已启用 · 第 1 位");
  await openPack("签名");
  expect((screen.getByRole("switch", { name: "启用" }) as HTMLInputElement).checked).toBe(true);
});

test("edits the local @ list and refuses a malformed reading before saving", async () => {
  const client = fakeClient();
  renderSection({ client });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "@ 名单" }).textContent).toContain("1 条"),
  );
  openMentions();
  expect(await screen.findByDisplayValue("张三")).toBeTruthy();
  expect(screen.getByText(/名单只保存在本机，不随账号同步/)).toBeTruthy();
  const save = screen.getByRole("button", { name: "保存名单" }) as HTMLButtonElement;
  expect(save.disabled).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  const names = screen.getAllByLabelText("名字或地点");
  const keys = screen.getAllByLabelText("拼音");
  fireEvent.change(names[1], { target: { value: " 北京 " } });
  fireEvent.change(keys[1], { target: { value: "Bei Jing" } });
  expect(screen.getByRole("alert").textContent).toContain("第 2 行的拼音只能是小写字母");
  expect(save.disabled).toBe(true);

  fireEvent.change(keys[1], { target: { value: "bei'jing" } });
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(save);
  await waitFor(() =>
    expect(client.saveMentions).toHaveBeenCalledWith([
      { text: "张三", key: "zhang'san" },
      { text: "北京", key: "bei'jing" },
    ]),
  );
  await waitFor(() => expect(save.disabled).toBe(true));

  fireEvent.click(screen.getByRole("button", { name: "删除第 1 行" }));
  expect(screen.queryByDisplayValue("张三")).toBeNull();
  // Unsaved edits survive leaving the view, and the entry says so.
  backToList();
  expect(screen.getByRole("button", { name: "@ 名单" }).textContent).toContain("有未保存的修改");
  openMentions();
  expect(screen.getByDisplayValue("北京")).toBeTruthy();
});

test("checks a name list the way the host does", () => {
  expect(mentionListIssue([{ text: "张三", key: "" }])).toBeNull();
  expect(mentionListIssue([{ text: "Alice", key: "" }])).toBeNull();
  expect(mentionListIssue([{ text: " ", key: "" }])).toBe("第 1 行还没有填写名字或地点。");
  expect(mentionListIssue([{ text: "x".repeat(200), key: "" }])).toBe("第 1 行太长了。");
  expect(mentionListIssue([{ text: "a\u0007", key: "" }])).toBe("第 1 行含有控制字符。");
  expect(mentionListIssue([{ text: "张三", key: "zhang''san" }])).toContain("第 1 行的拼音");
  expect(mentionListIssue([{ text: "张三", key: "a".repeat(65) }])).toContain("第 1 行的拼音");
  expect(
    mentionListIssue([
      { text: "张三", key: "" },
      { text: "张三", key: "zs" },
    ]),
  ).toBe("「张三」重复了。");
  // Names are saved trimmed, so a pasted trailing space does not make a second name.
  expect(
    mentionListIssue([
      { text: "张三", key: "" },
      { text: "张三 ", key: "" },
    ]),
  ).toBe("「张三」重复了。");
  expect(
    mentionListIssue(Array.from({ length: 1001 }, (_, index) => ({ text: `${index}`, key: "" }))),
  ).toBe("名单最多 1000 条。");
});

test("decodes host failures, falling back for unknown ones", () => {
  expect(pluginErrorMessage({ code: "plugin_reserved" }, "失败")).toBe(
    "这个 id 属于内置插件，不能覆盖或删除。",
  );
  expect(pluginErrorMessage({ code: "plugin_archive", detail: "压缩包里的文件太多" }, "失败")).toBe(
    "压缩包无法读取（压缩包里的文件太多）。",
  );
  expect(pluginErrorMessage({ code: "mention_invalid", detail: null }, "失败")).toBe("名单有误。");
  expect(pluginErrorMessage(new Error("boom"), "失败")).toBe("失败");
});

test("hides the typing effects where the host draws none", () => {
  renderSection({ client: undefined });
  openSoundEffects();
  expect(screen.queryByText("打字效果")).toBeNull();
  expect(screen.queryByRole("switch", { name: "连击计数" })).toBeNull();
});

test("switches the typing effect style, intensity, combo count and tier sound", () => {
  const { onChange } = renderSection({
    client: undefined,
    typingEffects: true,
    effectStyles: true,
  });
  openSoundEffects();
  const intensity = screen.getByRole("slider", { name: "效果强度" }) as HTMLInputElement;
  // Nothing is drawn while the style is off, so its size has nothing to act on.
  expect(intensity.disabled).toBe(true);
  expect(
    within(screen.getByRole("radiogroup", { name: "效果样式" }))
      .getAllByRole("radio")
      .map((radio) => radio.closest("label")?.textContent),
  ).toEqual(["关闭", "闪光", "火花", "Power Mode"]);

  fireEvent.click(screen.getByRole("radio", { name: "Power Mode" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    effect_style: "power_mode",
  });

  fireEvent.click(screen.getByRole("switch", { name: "连击计数" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...defaultPluginPreferences, combo_counter: true });
  // The tier sound marks a combo tier, so it waits for the counter.
  expect((screen.getByRole("switch", { name: "升档音" }) as HTMLInputElement).disabled).toBe(true);

  cleanup();
  const on: PluginPreferences = {
    ...defaultPluginPreferences,
    effect_style: "sparks",
    combo_counter: true,
  };
  const next = renderSection({ client: undefined, typingEffects: true, effectStyles: true }, on);
  openSoundEffects();
  fireEvent.change(screen.getByRole("slider", { name: "效果强度" }), { target: { value: "80" } });
  expect(next.onChange).toHaveBeenLastCalledWith({ ...on, effect_intensity: 80 });
  fireEvent.click(screen.getByRole("switch", { name: "升档音" }));
  expect(next.onChange).toHaveBeenLastCalledWith({ ...on, combo_tier_sound: true });
});

test("selects an installed effect pack, which takes over the style and intensity", async () => {
  const effects = fakeClient({
    catalog: vi.fn(async () => ({
      packages: [
        ...catalog.packages,
        pack({ id: "neon", kind: "effect", name: "霓虹", style: "sparks" }),
      ],
      issues: [],
    })),
  });
  const { onChange } = renderSection({
    client: effects,
    typingEffects: true,
    effectStyles: true,
    effectPacks: true,
  });
  await openPack("霓虹");
  expect(screen.getByText("火花")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "使用此特效包" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...defaultPluginPreferences, effect_pack: "neon" });

  cleanup();
  const using = {
    ...defaultPluginPreferences,
    effect_style: "flash" as const,
    effect_pack: "neon",
  };
  const next = renderSection(
    { client: effects, typingEffects: true, effectStyles: true, effectPacks: true },
    using,
  );
  expect((await screen.findByRole("button", { name: "霓虹" })).textContent).toContain("使用中");
  openSoundEffects();
  expect(screen.getByText("霓虹")).toBeTruthy();
  expect((screen.getByRole("radio", { name: "闪光" }) as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByRole("slider", { name: "效果强度" }) as HTMLInputElement).disabled).toBe(
    true,
  );
  backToList();
  await openPack("霓虹");
  fireEvent.click(screen.getByRole("button", { name: "停用" }));
  expect(next.onChange).toHaveBeenLastCalledWith({ ...using, effect_pack: "" });

  cleanup();
  renderSection({ client: effects, typingEffects: true, effectStyles: true, effectPacks: false });
  await openPack("霓虹");
  expect(screen.getByText("这台设备不绘制特效包。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "使用此特效包" })).toBeNull();
  backToList();
  openSoundEffects();
  expect(screen.getByRole("radiogroup", { name: "效果样式" })).toBeTruthy();
  expect(screen.queryByText("特效包")).toBeNull();
});

test("offers only the combo count where the host draws no effect styles", () => {
  renderSection({ client: undefined, typingEffects: true, effectStyles: false });
  openSoundEffects();
  expect(screen.getByRole("switch", { name: "连击计数" })).toBeTruthy();
  expect(screen.queryByRole("radiogroup", { name: "效果样式" })).toBeNull();
  expect(screen.queryByRole("slider", { name: "效果强度" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "升档音" })).toBeNull();
});

test("lists the built-in packs by their Chinese names, each under its own kind", async () => {
  const builtin: PluginCatalogResult = {
    packages: [
      pack({ id: "default", kind: "sound", name: "清脆键盘", builtin: true, mode: "keys" }),
      pack({ id: "msime-typewriter", kind: "sound", name: "打字机", builtin: true, mode: "keys" }),
      pack({ id: "msime-8bit", kind: "sound", name: "8 位游戏机", builtin: true, mode: "keys" }),
      pack({ id: "twinkle", kind: "sound", name: "小星星", builtin: true, mode: "sequence" }),
      pack({ id: "msime-canon", kind: "sound", name: "卡农", builtin: true, mode: "sequence" }),
      pack({
        id: "msime-music-lofi",
        kind: "music",
        name: "Lo-fi 午后",
        builtin: true,
        tracks: ["lofi.wav"],
      }),
    ],
    issues: [],
  };
  const client = fakeClient({ catalog: vi.fn(async () => builtin) });
  renderSection(
    { client },
    {
      ...defaultPluginPreferences,
      key_sound: { ...defaultPluginPreferences.key_sound, mode: "melody" },
    },
  );
  const list = await screen.findByLabelText("已安装的插件");
  await within(list).findByRole("button", { name: "卡农" });
  const names = (kind: string) =>
    within(within(list).getByRole("region", { name: kind }))
      .getAllByRole("button")
      .map((row) => row.getAttribute("aria-label"));
  expect(names("音效包")).toEqual(["清脆键盘", "打字机", "8 位游戏机", "小星星", "卡农"]);
  expect(names("音乐包")).toEqual(["Lo-fi 午后"]);

  openSoundEffects();
  expect(screen.getByText("清脆键盘（内置）")).toBeTruthy();
  expect(screen.getByText("小星星（内置）")).toBeTruthy();
  expect(screen.getByText("未选择")).toBeTruthy();
  backToList();

  // Built-in music cannot be removed any more than the built-in sound packs.
  await openPack("Lo-fi 午后");
  expect(screen.getByText("lofi.wav")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "删除Lo-fi 午后" })).toBeNull();
});

test("fills the defaults the document leaves out and forgets removed packs", () => {
  expect(pluginPreferences({})).toEqual(defaultPluginPreferences);
  // `PluginPreferences::default()` in client-core: no effect, half intensity, no combo.
  expect(defaultPluginPreferences).toMatchObject({
    effect_style: "off",
    effect_intensity: 50,
    effect_pack: "",
    combo_counter: false,
    combo_tier_sound: false,
  });
  // The selected effect pack survives a save, which rebuilds the section field by field.
  expect(
    pluginPreferences({
      plugins: { effect_pack: "neon" } as unknown as Preferences["plugins"],
    }).effect_pack,
  ).toBe("neon");
  // A document written before the effect fields reads them as their defaults, and one that has them keeps them.
  expect(
    pluginPreferences({
      plugins: { combo_counter: true, effect_style: "sparks" } as unknown as Preferences["plugins"],
    }),
  ).toEqual({ ...defaultPluginPreferences, combo_counter: true, effect_style: "sparks" });
  expect(
    pluginPreferences({
      plugins: { music: { enabled: true } } as unknown as Preferences["plugins"],
    }).music,
  ).toEqual({ enabled: true, pack: "", volume: 30 });

  const selected: PluginPreferences = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "custom" },
    melody: { pack: "custom" },
    music: { enabled: true, pack: "rain", volume: 40 },
    command_tables: ["a", "b"],
    effect_pack: "neon",
  };
  expect(withoutRemovedPack(selected, "sound", "custom")).toEqual({
    ...selected,
    key_sound: { ...selected.key_sound, pack: "default" },
    melody: { pack: "twinkle" },
  });
  expect(withoutRemovedPack(selected, "music", "rain").music).toEqual({
    enabled: false,
    pack: "",
    volume: 40,
  });
  expect(withoutRemovedPack(selected, "music", "other")).toBe(selected);
  expect(withoutRemovedPack(selected, "command_table", "a").command_tables).toEqual(["b"]);
  expect(withoutRemovedPack(selected, "effect", "neon")).toEqual({ ...selected, effect_pack: "" });
  expect(withoutRemovedPack(selected, "effect", "other")).toBe(selected);
  // A removed sound pack of the same id leaves the effect selection alone.
  expect(withoutRemovedPack(selected, "sound", "neon").effect_pack).toBe("neon");

  expect(withPackSelected(selected, { kind: "sound", id: "piano", mode: "keys" })).toEqual({
    ...selected,
    key_sound: { ...selected.key_sound, pack: "piano" },
  });
  expect(withPackSelected(selected, { kind: "sound", id: "canon", mode: "sequence" })).toEqual({
    ...selected,
    melody: { pack: "canon" },
  });
  // Choosing music leaves whether it plays as it was.
  expect(withPackSelected(selected, { kind: "music", id: "sea" }).music).toEqual({
    enabled: true,
    pack: "sea",
    volume: 40,
  });
  expect(withPackSelected(selected, { kind: "effect", id: "fire" }).effect_pack).toBe("fire");
  expect(withPackSelected(selected, { kind: "command_table", id: "c" }).command_tables).toEqual([
    "a",
    "b",
    "c",
  ]);
  expect(withPackSelected(selected, { kind: "command_table", id: "a" })).toBe(selected);
});

test("shows the V, / and @ switches only where the host routes them", () => {
  const onChange = vi.fn();
  render(<LocalModesSection preferences={defaultLocalModes} ios={false} onChange={onChange} />);
  expect(screen.queryByRole("switch", { name: /V 模式/ })).toBeNull();

  cleanup();
  // A document written while the three were off leaves them out, as the defaults do.
  const stored = defaultLocalModes;
  expect("expression" in stored).toBe(false);
  render(<LocalModesSection preferences={stored} ios={false} triggers onChange={onChange} />);
  const expression = screen.getByRole("switch", { name: /V 模式/ }) as HTMLInputElement;
  expect(expression.checked).toBe(false);
  expect(screen.getByRole("switch", { name: /\/ 模式/ })).toBeTruthy();
  // Without a way to edit the @ list (a host with no plugin store), the @ mode could never produce a candidate, so its switch is not offered.
  expect(screen.queryByRole("switch", { name: /@ 模式/ })).toBeNull();

  cleanup();
  render(
    <LocalModesSection preferences={stored} ios={false} triggers mentions onChange={onChange} />,
  );

  fireEvent.click(screen.getByRole("switch", { name: /@ 模式/ }));
  expect(onChange).toHaveBeenCalledWith({ ...stored, mention: true });
});

test("offers the built-in places under the @ switch, switchable only while @ is on", () => {
  const onChange = vi.fn();
  render(
    <LocalModesSection preferences={defaultLocalModes} ios={false} triggers onChange={onChange} />,
  );
  // No @ switch, so no places either.
  expect(screen.queryByRole("switch", { name: "@ 地名" })).toBeNull();

  cleanup();
  render(
    <LocalModesSection
      preferences={defaultLocalModes}
      ios={false}
      triggers
      mentions
      onChange={onChange}
    />,
  );
  const off = screen.getByRole("switch", { name: "@ 地名" }) as HTMLInputElement;
  expect(off.checked).toBe(false);
  expect(off.disabled).toBe(true);

  cleanup();
  const mention = { ...defaultLocalModes, mention: true };
  render(
    <LocalModesSection preferences={mention} ios={false} triggers mentions onChange={onChange} />,
  );
  const places = screen.getByRole("switch", { name: "@ 地名" }) as HTMLInputElement;
  expect(places.disabled).toBe(false);
  fireEvent.click(places);
  expect(onChange).toHaveBeenLastCalledWith({ ...mention, mention_places: true });
});

test("says /fy needs a translation service only while none is chosen", () => {
  const notice = /fy 翻译需要先在「表达 → 候选词翻译」选择翻译服务/;
  render(
    <LocalModesSection
      preferences={defaultLocalModes}
      ios={false}
      triggers
      translationService={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.getByText(notice)).toBeTruthy();

  cleanup();
  render(
    <LocalModesSection
      preferences={defaultLocalModes}
      ios={false}
      triggers
      translationService
      onChange={vi.fn()}
    />,
  );
  expect(screen.getByText(/fy 翻译（fy 后输入英文/)).toBeTruthy();
  expect(screen.queryByText(notice)).toBeNull();
});

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

test("the 插件 page saves plugin settings into the preferences document", async () => {
  const save = vi.fn(async (_revision: number, preferences: Preferences) => ({
    ...snapshot,
    revision: 4,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save,
        host: { platform: "macos", key_sound: true, music: true, plugin_triggers: true } as never,
        plugins: fakeClient(),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  const form = screen.getByRole("group", { name: "插件" });
  await within(form).findByRole("button", { name: "打字机" });

  fireEvent.click(within(form).getByRole("button", { name: "声音与效果" }));
  fireEvent.click(within(form).getByRole("switch", { name: "按键音" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].plugins?.key_sound.enabled).toBe(true);
});

test("the 插件 page offers the typing effects where the host draws them, and only the combo count on Linux", async () => {
  const save = vi.fn(async (_revision: number, preferences: Preferences) => ({
    ...snapshot,
    revision: 4,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save,
        host: { platform: "macos", typing_effects: true } as never,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  const form = screen.getByRole("group", { name: "插件" });
  // No pack store on this host, and the switches are still one step away.
  fireEvent.click(within(form).getByRole("button", { name: "声音与效果" }));
  expect(within(form).getByRole("radiogroup", { name: "效果样式" })).toBeTruthy();
  fireEvent.click(within(form).getByRole("radio", { name: "火花" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].plugins?.effect_style).toBe("sparks");

  cleanup();
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "linux", typing_effects: true } as never,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "插件" }));
  const linux = screen.getByRole("group", { name: "插件" });
  fireEvent.click(within(linux).getByRole("button", { name: "声音与效果" }));
  expect(within(linux).getByRole("switch", { name: "连击计数" })).toBeTruthy();
  expect(within(linux).queryByRole("radiogroup", { name: "效果样式" })).toBeNull();
  expect(within(linux).queryByRole("switch", { name: "升档音" })).toBeNull();
});

test("effect packs are offered where the host draws a style, not on Linux", () => {
  const capabilities = (platform: "macos" | "windows" | "harmony" | "linux") =>
    settingsCapabilities({
      host: { platform, typing_effects: true } as never,
      linux: platform === "linux",
      android: false,
      ios: false,
      harmony: platform === "harmony",
      windows: platform === "windows",
      macos: platform === "macos",
      mobile: platform === "harmony",
      canRestartInputMethod: false,
      canInstallInputSource: false,
      canListVoiceCaptureDevices: false,
    });
  expect(capabilities("macos").showTypingEffectPacks).toBe(true);
  expect(capabilities("windows").showTypingEffectPacks).toBe(true);
  expect(capabilities("harmony").showTypingEffectStyles).toBe(true);
  expect(capabilities("harmony").showTypingEffectPacks).toBe(true);
  expect(capabilities("linux").showTypingEffectPacks).toBe(false);
});

test("the @ switch is offered only where the host can edit the name list", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "macos", plugin_triggers: true } as never,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const form = screen.getByRole("group", { name: "输入" });
  expect(within(form).getByRole("switch", { name: /\/ 模式/ })).toBeTruthy();
  expect(within(form).queryByRole("switch", { name: /@ 模式/ })).toBeNull();
});

test("the 插件 page is not offered on a phone or a host that backs none of it", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "macos" } as never,
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "插件" })).toBeNull();

  cleanup();
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "android", key_sound: true, plugin_triggers: true } as never,
        plugins: fakeClient(),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "插件" })).toBeNull();
});
