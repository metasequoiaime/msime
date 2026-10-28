// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  CommunityPluginsPage,
  CommunityResourcesPage,
  CommunitySkinsPage,
  SettingsPage,
  ToastProvider,
  type CommunityPlugin,
  type CommunityPluginClient,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunitySkin,
  type CommunitySkinClient,
  type CustomSkinLibraryClient,
  type Snapshot,
} from "@msime/ui";
import { settingsCapabilities } from "../../../../packages/ui/src/settings/settings-capabilities";

// 鸿蒙版在实名认证到位之前，宿主报 `community_contribution` 为 false（`crates/client-core/src/host_surface.rs`）：社区页不画发布、改分类、发布新版本、评分和举报，浏览、获取、安装、收藏、导入和下架自己的作品照常；其他宿主不变。

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const design = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0 as const,
  monospaced: false,
};

const OTHERS = "10000000-0000-4000-8000-000000000001";
const MINE = "10000000-0000-4000-8000-000000000002";

function skin(id: string, owned: boolean): CommunitySkin {
  return {
    id,
    name: owned ? "我的皮肤" : "别人的皮肤",
    description: "合成说明",
    author: "示例作者",
    design,
    downloads: 3,
    rating_count: 1,
    rating_average: 4,
    owned,
    my_rating: 0,
    category: "nature",
  };
}

function skinClient(): CommunitySkinClient {
  return {
    list: vi
      .fn()
      .mockResolvedValue({ skins: [skin(OTHERS, false), skin(MINE, true)], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => skin(id, id === MINE)),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    setCategory: vi.fn(),
    finishTrial: vi.fn(),
    report: vi.fn(),
  };
}

function library(): CustomSkinLibraryClient {
  return { load: vi.fn().mockResolvedValue([]), mutate: vi.fn() };
}

function resource(id: string, owned: boolean): CommunityResource {
  return {
    id,
    kind: "dictionary",
    name: owned ? "我的词包" : "别人的词包",
    description: "合成说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", code: "hc", word: "合成", weight: 1 }] },
    revision: 1,
    saves: 0,
    saved: false,
    owned,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}

function resourceClient(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({
      items: [resource(OTHERS, false), resource(MINE, true)],
      has_more: false,
    }),
    detail: vi.fn().mockImplementation(async (id: string) => resource(id, id === MINE)),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
    report: vi.fn(),
  };
}

function plugin(id: string, owned: boolean): CommunityPlugin {
  return {
    id,
    kind: "sound",
    plugin_id: "rain",
    name: owned ? "我的插件" : "别人的插件",
    description: "合成说明",
    author: "示例作者",
    version: "1.0.0",
    license: "CC-BY-4.0",
    size: 2048,
    sha256: "0".repeat(64),
    downloads: 3,
    rating_count: 1,
    rating_average: 4,
    owned,
    my_rating: 0,
    created_at: "2026-09-30T00:00:00Z",
  };
}

function pluginClient(): CommunityPluginClient {
  return {
    list: vi
      .fn()
      .mockResolvedValue({ plugins: [plugin(OTHERS, false)], has_more: false, skipped: 0 }),
    detail: vi.fn().mockImplementation(async (id: string) => plugin(id, false)),
    packPreview: vi.fn(),
    publish: vi.fn(),
    install: vi.fn(),
    rate: vi.fn(),
    delete: vi.fn(),
    report: vi.fn(),
  };
}

const flags = {
  linux: false,
  android: false,
  ios: false,
  harmony: true,
  windows: false,
  macos: false,
  mobile: true,
  canRestartInputMethod: false,
  canInstallInputSource: false,
  canListVoiceCaptureDevices: false,
};

test("only an explicit false from the host withholds community contribution", () => {
  expect(
    settingsCapabilities({ ...flags, host: testHost({ platform: "harmony" }) })
      .showCommunityContribution,
  ).toBe(false);
  expect(
    settingsCapabilities({
      ...flags,
      host: testHost({ platform: "harmony", community_contribution: true }),
    }).showCommunityContribution,
  ).toBe(true);
  // 早于这一项的宿主不报它，网页预览没有宿主：都照旧给出。
  const { community_contribution: _omitted, ...older } = testHost({ platform: "macos" });
  expect(
    settingsCapabilities({ ...flags, harmony: false, macos: true, host: older })
      .showCommunityContribution,
  ).toBe(true);
  expect(settingsCapabilities({ ...flags, host: undefined }).showCommunityContribution).toBe(true);
  for (const platform of ["macos", "windows", "linux", "android", "ios"]) {
    expect(testHost({ platform }).community_contribution).toBe(true);
  }
});

test.each([undefined, "harmony"] as const)(
  "the skin gallery without contribution keeps browsing and drops publish, rating, category and report (look %s)",
  async (look) => {
    const client = skinClient();
    render(
      <ToastProvider>
        <CommunitySkinsPage
          client={client}
          theme="light"
          localSkinLibrary={library()}
          look={look}
          contributions={false}
        />
      </ToastProvider>,
    );
    await screen.findByRole("button", { name: "查看皮肤 别人的皮肤" });
    expect(screen.queryByRole("button", { name: /^发布(设计|我的设计)$/ })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "查看皮肤 别人的皮肤" }));
    expect(await screen.findByRole("button", { name: "下载并试用" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "评 5 星" })).toBeNull();
    expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
  },
);

test("an own skin without contribution can still be taken down but not recategorised", async () => {
  const client = skinClient();
  render(
    <ToastProvider>
      <CommunitySkinsPage client={client} theme="light" contributions={false} />
    </ToastProvider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 我的皮肤" }));
  expect(await screen.findByRole("button", { name: "下架这款皮肤" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "修改分类" })).toBeNull();
  expect(client.setCategory).not.toHaveBeenCalled();
});

test("the skin gallery with contribution keeps every entry", async () => {
  render(
    <ToastProvider>
      <CommunitySkinsPage client={skinClient()} theme="light" localSkinLibrary={library()} />
    </ToastProvider>,
  );
  expect(await screen.findByRole("button", { name: "发布我的设计" })).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 别人的皮肤" }));
  expect(await screen.findByRole("button", { name: "评 5 星" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "举报" })).toBeTruthy();
});

test.each([undefined, "harmony"] as const)(
  "resources without contribution keep save and import and drop publish, rating and report (look %s)",
  async (look) => {
    const client = resourceClient();
    render(
      <CommunityResourcesPage
        client={client}
        kind="dictionary"
        look={look}
        contributions={false}
      />,
    );
    await screen.findByRole("button", { name: "查看词库 别人的词包" });
    expect(screen.queryByRole("button", { name: "发布作品" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "查看词库 别人的词包" }));
    expect(await screen.findByRole("button", { name: "收藏，关注后续更新" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "导入这版词库到云端" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "评 5 星" })).toBeNull();
    expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
  },
);

test("an own resource without contribution offers no new version but can be taken down", async () => {
  render(
    <CommunityResourcesPage client={resourceClient()} kind="dictionary" contributions={false} />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 我的词包" }));
  expect(await screen.findByRole("button", { name: "下架作品" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "编辑并发布新版本" })).toBeNull();
});

test("resources with contribution keep publish, new versions, rating and report", async () => {
  render(<CommunityResourcesPage client={resourceClient()} kind="dictionary" />);
  expect(await screen.findByRole("button", { name: "发布作品" })).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 别人的词包" }));
  expect(await screen.findByRole("button", { name: "评 5 星" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "举报" })).toBeTruthy();
});

test("plugins without contribution keep install and drop publish, rating and report", async () => {
  const client = pluginClient();
  const localPlugins = vi.fn().mockResolvedValue({ packages: [], issues: [] });
  render(
    <CommunityPluginsPage client={client} localPlugins={localPlugins} contributions={false} />,
  );
  await screen.findByRole("button", { name: "查看插件 别人的插件" });
  expect(screen.queryByRole("button", { name: "发布我的插件" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "查看插件 别人的插件" }));
  await waitFor(() => expect(client.detail).toHaveBeenCalled());
  expect(await screen.findByRole("button", { name: "一键安装" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "评 5 星" })).toBeNull();
  expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
  expect(client.packPreview).not.toHaveBeenCalled();
});

test("plugins with contribution keep publish, rating and report", async () => {
  const localPlugins = vi.fn().mockResolvedValue({ packages: [], issues: [] });
  render(<CommunityPluginsPage client={pluginClient()} localPlugins={localPlugins} />);
  expect(await screen.findByRole("button", { name: "发布我的插件" })).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 别人的插件" }));
  expect(await screen.findByRole("button", { name: "评 5 星" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "举报" })).toBeTruthy();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function renderHarmonyCommunity(fields: Record<string, unknown> = {}) {
  const communitySkins = skinClient();
  render(
    <SettingsPage
      initialPage="community"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        host: testHost({ platform: "harmony", ...fields }),
        communitySkins,
        customSkinLibrary: library(),
      }}
    />,
  );
  return communitySkins;
}

test("the HarmonyOS community tab follows the host capability", async () => {
  const withheld = renderHarmonyCommunity();
  await screen.findByRole("button", { name: "查看皮肤 别人的皮肤" });
  expect(withheld.list).toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "发布设计" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 别人的皮肤" }));
  expect(await screen.findByRole("button", { name: "下载并试用" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "评 5 星" })).toBeNull();
  expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
  cleanup();
  window.history.replaceState({}, "");

  renderHarmonyCommunity({ community_contribution: true });
  expect(await screen.findByRole("button", { name: "发布设计" })).toBeTruthy();
});
