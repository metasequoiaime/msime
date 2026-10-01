// @vitest-environment jsdom
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
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
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(within(list).getByRole("button", { name: "删除打字机" }));
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
  await screen.findByLabelText("已安装的插件");
  await act(async () => {
    resolveRemove();
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(onChange).not.toHaveBeenCalled();
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

test("switches key sounds and keeps the other effect settings", () => {
  const { onChange } = renderSection({ client: undefined });

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

test("offers the melody pack only in melody mode, and the mode only while key sounds are on", async () => {
  const on = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, enabled: true },
  };
  const { onChange } = renderSection({}, on);
  await screen.findByText("打字机 1.0.0");

  expect(screen.queryByRole("combobox", { name: "旋律" })).toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "按键旋律" }));
  expect(onChange).toHaveBeenLastCalledWith({
    ...on,
    key_sound: { ...on.key_sound, mode: "melody" },
  });

  cleanup();
  renderSection({}, { ...on, key_sound: { ...on.key_sound, mode: "melody" } });
  await screen.findByText("打字机 1.0.0");
  const melody = screen.getByRole("combobox", { name: "旋律" });
  expect(
    within(melody)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["小星星（内置）"]);
  const packs = screen.getByRole("combobox", { name: "音效包" });
  expect(
    within(packs)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["默认（内置）", "打字机"]);

  cleanup();
  renderSection({ client: undefined });
  expect(
    screen.getByRole("radiogroup", { name: "发声方式" }).querySelector("input")?.disabled,
  ).toBe(true);
});

test("names a selected pack that is no longer installed instead of showing another", async () => {
  renderSection(
    {},
    {
      ...defaultPluginPreferences,
      key_sound: { ...defaultPluginPreferences.key_sound, pack: "gone" },
    },
  );
  await screen.findByText("打字机 1.0.0");
  const packs = screen.getByRole("combobox", { name: "音效包" }) as HTMLSelectElement;
  expect(packs.value).toBe("gone");
  expect(within(packs).getByRole("option", { name: "gone（未找到）" })).toBeTruthy();

  cleanup();
  // A host with no pack store lists nothing, so the selection is shown as it is rather than as missing.
  renderSection({ client: undefined });
  const selected = screen.getByRole("combobox", { name: "音效包" });
  expect(
    within(selected)
      .getAllByRole("option")
      .map((option) => option.textContent),
  ).toEqual(["default"]);
});

test("chooses a music pack, which starts unselected", async () => {
  const { onChange } = renderSection();
  await screen.findByText("雨声 1.0.0");
  const select = screen.getByRole("combobox", { name: "音乐包" }) as HTMLSelectElement;
  expect(select.value).toBe("");

  fireEvent.change(select, { target: { value: "rain" } });
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    music: { ...defaultPluginPreferences.music, pack: "rain" },
  });
});

test("says built-in commands still work where command tables cannot be imported", () => {
  renderSection({ client: undefined });
  expect(
    screen.getByText("这台设备还不能导入指令表，内置的 rq、sj、xq 指令照常可用。"),
  ).toBeTruthy();
  expect(screen.queryByText("还没有导入指令表。")).toBeNull();
  expect(screen.queryByText("@ 名单")).toBeNull();
});

test("does not promise a password-field pause the Windows host cannot make", () => {
  renderSection();
  expect(
    screen.getByText("默认关闭。只在输入法处于活动状态时播放，切换到其他输入法时暂停。"),
  ).toBeTruthy();
});

test("hides the groups a host does not back", () => {
  renderSection({ client: undefined, keySound: false, music: false, triggers: false });
  expect(screen.queryByRole("switch", { name: "按键音" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "背景音乐" })).toBeNull();
  expect(screen.queryByText("指令表")).toBeNull();
  expect(screen.queryByText("@ 名单")).toBeNull();
  expect(screen.queryByRole("button", { name: "导入文件夹" })).toBeNull();
});

test("lists every pack with its licence and removes only installed ones", async () => {
  const selected = {
    ...defaultPluginPreferences,
    key_sound: { ...defaultPluginPreferences.key_sound, pack: "typewriter" },
  };
  const { client, confirm, onChange } = renderSection({}, selected);
  const list = await screen.findByLabelText("已安装的插件");

  await waitFor(() => expect(within(list).getByText("打字机 1.0.0")).toBeTruthy());
  expect(within(list).getByText("音效包 · 作者 测试者 · 许可证 CC0-1.0")).toBeTruthy();
  expect(within(list).getAllByText("音效包 · 内置 · 许可证 CC0-1.0")).toHaveLength(2);
  expect(within(list).getByText("音乐包 · 许可证 CC-BY-4.0")).toBeTruthy();
  expect(within(list).queryByRole("button", { name: "删除默认" })).toBeNull();
  expect(screen.getByText("音效包 broken 无法载入：缺少 plugin.toml")).toBeTruthy();

  fireEvent.click(within(list).getByRole("button", { name: "删除打字机" }));
  await waitFor(() => expect(client!.remove).toHaveBeenCalledWith("sound", "typewriter"));
  expect(confirm).toHaveBeenCalledWith(expect.objectContaining({ danger: true }));
  // The selection falls back to the built-in pack rather than naming one that is gone.
  expect(onChange).toHaveBeenCalledWith(defaultPluginPreferences);
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
  fireEvent.click(await screen.findByRole("button", { name: "删除打字机" }));
  await waitFor(() => expect(client.remove).not.toHaveBeenCalled());
});

test("imports through the host picker and reports what was installed", async () => {
  const imported = pack({ id: "piano", kind: "sound", name: "钢琴", version: "2.0.0" });
  const client = fakeClient({ importPack: vi.fn(async () => imported) });
  renderSection({ client });
  await screen.findByText("打字机 1.0.0");

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
  await screen.findByText("打字机 1.0.0");

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
  await screen.findByText("打字机 1.0.0");
  fireEvent.click(screen.getByRole("button", { name: "导入文件夹" }));
  await waitFor(() => expect(client.importPack).toHaveBeenCalled());
  expect(client.catalog).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("status")).toBeNull();
});

test("enables command tables in order and lists enabled tables that are gone", async () => {
  const { onChange } = renderSection(
    {},
    { ...defaultPluginPreferences, command_tables: ["removed"] },
  );
  const table = await screen.findByRole("switch", { name: "签名" });
  expect(screen.getByText("1 条指令：/sig 签名")).toBeTruthy();

  fireEvent.click(table);
  expect(onChange).toHaveBeenLastCalledWith({
    ...defaultPluginPreferences,
    command_tables: ["removed", "signature"],
  });

  fireEvent.click(screen.getByRole("switch", { name: "removed（未找到）" }));
  expect(onChange).toHaveBeenLastCalledWith({ ...defaultPluginPreferences, command_tables: [] });
});

test("edits the local @ list and refuses a malformed reading before saving", async () => {
  const client = fakeClient();
  renderSection({ client });
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
  expect(screen.queryByText("打字效果")).toBeNull();
  expect(screen.queryByRole("switch", { name: "连击计数" })).toBeNull();
});

test("switches the typing effect style, intensity, combo count and tier sound", () => {
  const { onChange } = renderSection({
    client: undefined,
    typingEffects: true,
    effectStyles: true,
  });
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
  const select = (await screen.findByRole("combobox", { name: "特效包" })) as HTMLSelectElement;
  await within(select).findByRole("option", { name: "霓虹" });
  fireEvent.change(select, { target: { value: "neon" } });
  expect(onChange).toHaveBeenLastCalledWith({ ...defaultPluginPreferences, effect_pack: "neon" });

  cleanup();
  renderSection(
    { client: effects, typingEffects: true, effectStyles: true, effectPacks: true },
    { ...defaultPluginPreferences, effect_style: "flash", effect_pack: "neon" },
  );
  await within(screen.getByRole("combobox", { name: "特效包" })).findByRole("option", {
    name: "霓虹",
  });
  expect((screen.getByRole("radio", { name: "闪光" }) as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByRole("slider", { name: "效果强度" }) as HTMLInputElement).disabled).toBe(
    true,
  );

  cleanup();
  renderSection({ client: effects, typingEffects: true, effectStyles: true, effectPacks: false });
  expect(screen.getByRole("radiogroup", { name: "效果样式" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "特效包" })).toBeNull();
});

test("offers only the combo count where the host draws no effect styles", () => {
  renderSection({ client: undefined, typingEffects: true, effectStyles: false });
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
  await screen.findByText("卡农 1.0.0");
  const options = (name: string) =>
    within(screen.getByRole("combobox", { name }))
      .getAllByRole("option")
      .map((option) => option.textContent);
  expect(options("音效包")).toEqual(["清脆键盘（内置）", "打字机（内置）", "8 位游戏机（内置）"]);
  expect(options("旋律")).toEqual(["小星星（内置）", "卡农（内置）"]);
  expect(options("音乐包")).toEqual(["未选择", "Lo-fi 午后（内置）"]);
  // Built-in music cannot be removed any more than the built-in sound packs.
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
  await within(form).findByText("打字机 1.0.0");

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
