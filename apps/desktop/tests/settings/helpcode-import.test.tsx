// @vitest-environment jsdom
import { useState } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  HelpcodeSettingsPage,
  SettingsPage,
  defaultPluginPreferences,
  useHelpcodePacks,
  type PluginClient,
  type PluginPackage,
  type Preferences,
} from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow, selectSettingsTab } from "../support/settings-form";
import { useHelpcodeImport } from "../../../../packages/ui/src/settings/use-helpcode-import";

afterEach(cleanup);
const pack: PluginPackage = {
  id: "custom-synthetic",
  name: "合成码表",
  kind: "helpcode",
  version: "1",
  license: "NOASSERTION",
  author: null,
  description: null,
  builtin: false,
};
const initial: Preferences = {
  scheme: "shuangpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
  plugins: {
    ...defaultPluginPreferences,
    helpcode_pack_quanpin: "keep",
    key_sound: { enabled: false, mode: "keys", pack: "sound", volume: 50 },
  },
};
function deferred() {
  let resolve!: (value: PluginPackage | null) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<PluginPackage | null>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}
function client(importHelpcodeTable: PluginClient["importHelpcodeTable"]): PluginClient {
  return {
    importHelpcodeTable,
    catalog: vi.fn(async () => ({ packages: [], issues: [] })),
    importPack: vi.fn(),
    remove: vi.fn(),
    loadMentions: vi.fn(),
    saveMentions: vi.fn(),
  };
}
function setup(plugins: PluginClient) {
  const refresh = vi.fn();
  const hook = renderHook(
    ({ host }) => {
      const [draft, setDraft] = useState<Preferences | undefined>(initial);
      return { ...useHelpcodeImport(host, setDraft, refresh), draft, setDraft };
    },
    { initialProps: { host: plugins } },
  );
  return { ...hook, refresh };
}

test("导入绑定发起组并在最新偏好上合并，重复点击只打开一次", async () => {
  const pending = deferred();
  const plugins = client(vi.fn(() => pending.promise));
  const { result, refresh } = setup(plugins);
  let task!: Promise<void>;
  act(() => {
    task = result.current.importTable!("shuangpin_helpcode");
    void result.current.importTable!("quanpin_helpcode");
  });
  expect(plugins.importHelpcodeTable).toHaveBeenCalledTimes(1);
  expect(result.current.busy).toBe(true);
  act(() =>
    result.current.setDraft((old) => ({
      ...initial,
      ...old,
      scheme: "quanpin",
      plugins: {
        ...defaultPluginPreferences,
        ...old?.plugins,
        key_sound: { enabled: false, mode: "keys", pack: "changed", volume: 50 },
      },
    })),
  );
  await act(async () => {
    pending.resolve(pack);
    await task;
  });
  expect(result.current.draft).toMatchObject({
    scheme: "quanpin",
    plugins: {
      helpcode_pack_quanpin: "keep",
      helpcode_pack_shuangpin: pack.id,
      key_sound: { enabled: false, mode: "keys", pack: "changed", volume: 50 },
    },
  });
  expect(refresh).toHaveBeenCalledOnce();
  expect(result.current.status).toContain("已选为双拼辅助码");
  expect(result.current.busy).toBe(false);
});

test.each(["cancel", "invalid"])("%s 保留原选择且不刷新目录", async (outcome) => {
  const plugins = client(
    vi.fn(async () => {
      if (outcome === "invalid") throw { code: "plugin_invalid", detail: "第 2 行重复" };
      return null;
    }),
  );
  const { result, refresh } = setup(plugins);
  await act(async () => {
    await result.current.importTable!("quanpin_helpcode");
  });
  expect(result.current.draft).toEqual(initial);
  expect(refresh).not.toHaveBeenCalled();
  expect(result.current.busy).toBe(false);
  expect(result.current.status).toBe(outcome === "cancel" ? "" : "插件不符合要求（第 2 行重复）。");
});

test("旧宿主完成的导入不能写入新宿主，也不能结束新选择器", async () => {
  const old = deferred();
  const next = deferred();
  const { result, rerender, refresh } = setup(client(() => old.promise));
  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.importTable!("quanpin_helpcode");
  });
  rerender({ host: client(() => next.promise) });
  act(() => {
    second = result.current.importTable!("shuangpin_helpcode");
  });
  await act(async () => {
    old.resolve(pack);
    await first;
  });
  expect(result.current.draft).toEqual(initial);
  expect(refresh).not.toHaveBeenCalled();
  expect(result.current.busy).toBe(true);
  await act(async () => {
    next.resolve(null);
    await second;
  });
});

test("卸载后不写回导入结果", async () => {
  const pending = deferred();
  const { result, unmount, refresh } = setup(client(() => pending.promise));
  let task!: Promise<void>;
  act(() => {
    task = result.current.importTable!("quanpin_helpcode");
  });
  unmount();
  await act(async () => {
    pending.resolve(pack);
    await task;
  });
  expect(refresh).not.toHaveBeenCalled();
});

test.each([true, false])("导入后目录即时刷新并显示选中的码表，mobile=%s", async (mobile) => {
  let installed = false;
  const plugins = client(
    vi.fn(async () => {
      installed = true;
      return pack;
    }),
  );
  plugins.catalog = vi.fn(async () => ({ packages: installed ? [pack] : [], issues: [] }));
  function Page() {
    const [draft, setDraft] = useState<Preferences | undefined>(initial);
    const [revision, setRevision] = useState(0);
    const packs = useHelpcodePacks(plugins.catalog, true, revision);
    const importer = useHelpcodeImport(plugins, setDraft, () => setRevision((old) => old + 1));
    return (
      <HelpcodeSettingsPage
        value={draft!}
        mobile={mobile}
        activeFamily="shuangpin_helpcode"
        showShiftEntry={false}
        packs={packs}
        onChange={vi.fn()}
        onImport={importer.importTable}
        importBusy={importer.busy}
        importStatus={importer.status}
      />
    );
  }
  render(<Page />);
  await waitFor(() => expect(plugins.catalog).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "导入双拼辅助码表" }));
  await waitFor(() =>
    expect(
      (screen.getByRole("combobox", { name: "双拼辅助码方案" }) as HTMLSelectElement).value,
    ).toBe(`pack:${pack.id}`),
  );
  await waitFor(() =>
    expect(screen.getAllByRole("option", { name: "合成码表（插件）" }).length).toBeGreaterThan(0),
  );
  expect(screen.getByRole("status").textContent).toContain("已选为双拼");
});

test.each([true, false])("HarmonyOS 输入设置完成导入并保存发起组，mobile=%s", async (mobile) => {
  let installed = false;
  const plugins = client(
    vi.fn(async () => {
      installed = true;
      return pack;
    }),
  );
  plugins.catalog = vi.fn(async () => ({ packages: installed ? [pack] : [], issues: [] }));
  const save = vi.fn(async (revision: number, preferences: Preferences) => ({
    format_version: 1,
    revision: revision + 1,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="input"
      client={{
        host: testHost({ platform: "harmony", mobile_settings: mobile }),
        load: async () => ({ format_version: 1, revision: 7, preferences: initial }),
        save,
        plugins,
      }}
    />,
  );
  await settingsFormReady();
  selectSettingsTab("进阶");
  const importButton = screen.getByRole("button", { name: "导入双拼辅助码表" });
  // 表单挂载不代表加载已结束；禁用的 fieldset 内点击不会触发导入。
  await waitFor(() => expect(importButton.matches(":disabled")).toBe(false));
  await act(async () => {
    fireEvent.click(importButton);
  });
  if (mobile) {
    // 鸿蒙手机用选择面板，已选码表显示在行上，原生 select 只负责传递值。
    expect(
      await screen.findByRole("button", { name: /辅助码方案.*合成码表（插件）/ }),
    ).toBeTruthy();
  } else {
    await waitFor(() =>
      expect(screen.getAllByRole("option", { name: "合成码表（插件）" }).length).toBeGreaterThan(0),
    );
  }
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        plugins: expect.objectContaining({
          helpcode_pack_quanpin: "keep",
          helpcode_pack_shuangpin: pack.id,
          key_sound: expect.objectContaining({ pack: "sound" }),
        }),
      }),
    ),
  );
});
