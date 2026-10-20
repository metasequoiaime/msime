// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  PluginsSection,
  defaultPluginPreferences,
  type PluginClient,
  type PluginPackage,
} from "@msime/ui";

afterEach(cleanup);

const sound: PluginPackage = {
  id: "synthetic-sound",
  kind: "sound",
  name: "合成键盘",
  version: "1.0.0",
  author: "合成作者",
  description: "清脆的声音",
  builtin: true,
  license: "CC0-1.0",
};
const music: PluginPackage = {
  ...sound,
  id: "synthetic-music",
  kind: "music",
  name: "合成午后",
  author: "Synthetic Music",
  builtin: false,
  tracks: ["合成曲目"],
};
function mount(
  packages = [sound, music],
  overrides: Partial<PluginClient> = {},
  platform: "macos" | "linux" | "windows" = "macos",
) {
  const client: PluginClient = {
    catalog: vi.fn(async () => ({ packages, issues: [] })),
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => {}),
    loadMentions: vi.fn(async () => []),
    saveMentions: vi.fn(async () => {}),
    ...overrides,
  };
  const view = render(
    <PluginsSection
      platform={platform}
      client={client}
      preferences={{
        ...defaultPluginPreferences,
        key_sound: { ...defaultPluginPreferences.key_sound, pack: sound.id },
      }}
      keySound
      music
      triggers
      active
      onChange={vi.fn()}
      onError={vi.fn()}
      confirm={vi.fn(async () => true)}
    />,
  );
  return { client, ...view };
}

test("分类与搜索共同过滤插件，详情返回保留筛选并恢复卡片焦点", async () => {
  mount();
  await screen.findByRole("button", { name: sound.name });
  const filters = screen.getByRole("group", { name: "已安装插件类型" });
  fireEvent.click(within(filters).getByRole("button", { name: /音乐包/ }));
  expect(screen.queryByRole("button", { name: sound.name })).toBeNull();
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索已安装插件" }), {
    target: { value: "synthetic music" },
  });
  fireEvent.click(screen.getByRole("button", { name: music.name }));
  expect(await screen.findByRole("heading", { name: music.name })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  expect(screen.getByRole("searchbox")).toHaveProperty("value", "synthetic music");
  expect(
    within(screen.getByRole("group", { name: "已安装插件类型" }))
      .getByRole("button", { name: /音乐包/ })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("button", { name: music.name })),
  );
});

test("空结果保留常用入口、导入和目录问题，清空搜索后恢复插件", async () => {
  const { client } = mount(undefined, {
    catalog: vi.fn(async () => ({
      packages: [sound, music],
      issues: [{ kind: "sound" as const, folder: "synthetic-broken", reason: "合成清单不完整" }],
    })),
  });
  await screen.findByRole("button", { name: sound.name });
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "不存在的合成插件" } });
  expect(screen.getByText("没有匹配的插件，试试其他关键词。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "声音与效果" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "@ 名单" })).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "无法载入的插件" })).getByText(/synthetic-broken/),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "导入 .zip" }));
  await waitFor(() => expect(client.importPack).toHaveBeenCalledWith("archive"));
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "" } });
  expect(screen.getByRole("button", { name: sound.name })).toBeTruthy();
});

test("隐藏没有运行能力的脚本，同时允许从缺失插件卡片恢复默认", async () => {
  mount([{ ...sound, id: "synthetic-script", kind: "script", name: "不支持的合成脚本" }]);
  const missing = await screen.findByRole("button", { name: /synthetic-sound/ });
  expect(screen.queryByRole("button", { name: "不支持的合成脚本" })).toBeNull();
  fireEvent.click(missing);
  expect(await screen.findByRole("button", { name: /改回默认/ })).toBeTruthy();
});

test("删除当前分类最后一个插件后回到全部，焦点仍留在可用入口", async () => {
  const packages = [sound, music];
  mount(packages, {
    remove: vi.fn(async () => {
      packages.pop();
    }),
  });
  await screen.findByRole("button", { name: music.name });
  fireEvent.click(
    within(screen.getByRole("group", { name: "已安装插件类型" })).getByRole("button", {
      name: /音乐包/,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: music.name }));
  fireEvent.click(await screen.findByRole("button", { name: `删除${music.name}` }));
  await screen.findByRole("button", { name: sound.name });
  const filters = screen.getByRole("group", { name: "已安装插件类型" });
  expect(
    within(filters)
      .getByRole("button", { name: /全部/ })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  expect(within(filters).queryByRole("button", { name: /音乐包/ })).toBeNull();
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "声音与效果" })),
  );
});

test("Linux 与 macOS 一样用可筛选的插件库，Windows 仍是按类型分组的列表", async () => {
  const linux = mount([sound, music], {}, "linux");
  await screen.findByRole("button", { name: sound.name });
  expect(screen.getByRole("group", { name: "已安装插件类型" })).toBeTruthy();
  expect(screen.getByRole("searchbox", { name: "搜索已安装插件" })).toBeTruthy();
  linux.unmount();
  mount([sound, music], {}, "windows");
  await screen.findByRole("button", { name: new RegExp(sound.name) });
  expect(screen.queryByRole("searchbox", { name: "搜索已安装插件" })).toBeNull();
});
