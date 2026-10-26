// @vitest-environment jsdom
import { act, cleanup, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  CommunityPluginCard,
  communityPluginKindsFor,
  communityPluginUnderReview,
  useCommunityPluginGallery,
  type CommunityPlugin,
  type CommunityPluginClient,
  type PluginCatalogResult,
  type PluginPackage,
} from "@msime/ui";
import { CommunityPluginDetail } from "../../../../packages/ui/src/community/community-plugin-detail";
import { useCommunityPluginPublication } from "../../../../packages/ui/src/community/use-community-plugin-publication";
import { communityPluginMessage } from "../../../../packages/ui/src/community/community-helpers";

afterEach(cleanup);

function item(overrides: Partial<CommunityPlugin> = {}): CommunityPlugin {
  return {
    id: "30000000-0000-4000-8000-000000000001",
    kind: "script",
    plugin_id: "pangu",
    name: "合成脚本",
    description: "合成说明",
    author: "合成作者",
    version: "1.0.0",
    license: "MIT",
    size: 2048,
    sha256: "0".repeat(64),
    downloads: 0,
    rating_count: 0,
    rating_average: 0,
    owned: false,
    my_rating: 0,
    created_at: "2026-10-26T00:00:00Z",
    permissions: ["read_commit", "rewrite_commit"],
    ...overrides,
  };
}

function pack(kind: PluginPackage["kind"], id: string, builtin = false): PluginPackage {
  return {
    kind,
    id,
    builtin,
    name: `合成 ${id}`,
    version: "1.0.0",
    license: "MIT",
    author: null,
    description: null,
  };
}

const detailProps = (plugin: CommunityPlugin) => ({
  plugin,
  actionBusy: false,
  detailBusy: false,
  actionNotice: "",
  error: "",
  signInRequired: false,
  installed: false,
  confirmReplace: false,
  confirmUnpublish: false,
  onBack: vi.fn(),
  onInstall: vi.fn(),
  onCancelReplace: vi.fn(),
  onRate: vi.fn(),
  onRequestUnpublish: vi.fn(),
  onUnpublish: vi.fn(),
  onCancelUnpublish: vi.fn(),
});

test("脚本插件只在运行脚本的宿主上可以筛选和发布", () => {
  expect(communityPluginKindsFor(false)).not.toContain("script");
  expect(communityPluginKindsFor(true)).toContain("script");
  expect(communityPluginKindsFor(true).slice(0, -1)).toEqual(communityPluginKindsFor(false));
});

test("只有作者自己待审的脚本插件算审核中，数据类插件的 pending 当作已通过", () => {
  expect(communityPluginUnderReview(item({ owned: true, moderation: "pending" }))).toBe(true);
  expect(communityPluginUnderReview(item({ owned: true, moderation: "approved" }))).toBe(false);
  expect(communityPluginUnderReview(item({ owned: false, moderation: "pending" }))).toBe(false);
  expect(
    communityPluginUnderReview(item({ kind: "sound", owned: true, moderation: "pending" })),
  ).toBe(false);
});

test("脚本插件卡片列出申请的能力，作者待审时显示审核中", () => {
  const { rerender } = render(
    <CommunityPluginCard plugin={item({ owned: true, moderation: "pending" })} open={vi.fn()} />,
  );
  const card = screen.getByRole("button", { name: "查看插件 合成脚本" });
  expect(within(card).getByText("脚本插件 · 我的作品 · 审核中")).toBeTruthy();
  expect(within(card).getByText("申请：读取输入的文字、修改输入的文字")).toBeTruthy();

  rerender(
    <CommunityPluginCard
      plugin={item({ kind: "sound", owned: true, moderation: "pending", permissions: undefined })}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("音效包 · 我的作品")).toBeTruthy();
  expect(screen.queryByText(/申请：/)).toBeNull();
});

test("脚本插件详情用授权页的白话列出能力，标出改写上屏，并说明安装后不会运行", () => {
  render(
    <CommunityPluginDetail
      {...detailProps(item({ permissions: ["read_commit", "rewrite_commit", "persist_data"] }))}
    />,
  );
  const section = screen.getByRole("region", { name: "脚本插件申请的能力" });
  expect(within(section).getByText(/安装后不会运行，要在脚本插件页授权/)).toBeTruthy();
  expect(within(section).getByText("读取输入的文字")).toBeTruthy();
  expect(within(section).getByText("注意：修改输入的文字")).toBeTruthy();
  // 这个版本还不认识的能力原样列出，并提示装不上。
  expect(within(section).getByText("注意：未知能力 persist_data")).toBeTruthy();
  expect(within(section).getByText(/还不认识的能力，安装会失败/)).toBeTruthy();
});

test("作者待审的脚本插件详情说明先审后发，装好后提示去授权", () => {
  const { rerender } = render(
    <CommunityPluginDetail {...detailProps(item({ owned: true, moderation: "pending" }))} />,
  );
  expect(screen.getByText("我的作品 · 审核中")).toBeTruthy();
  expect(screen.getByText(/通过后才会出现在社区里/)).toBeTruthy();
  rerender(<CommunityPluginDetail {...detailProps(item())} installed />);
  expect(screen.getByText(/已安装，但还不会运行/)).toBeTruthy();
});

test("运行脚本的宿主发布时列出自己导入的脚本插件，官方脚本包不列", async () => {
  const localPlugins = vi.fn(async (): Promise<PluginCatalogResult> => ({
    packages: [pack("script", "uuid", true), pack("script", "pangu"), pack("sound", "rain")],
    issues: [],
  }));
  const client = {
    packPreview: vi.fn(async () => ({
      suggestedName: "合成",
      suggestedDescription: "",
      version: "1.0.0",
      license: "MIT",
      fileCount: 2,
      size: 1024,
    })),
    publish: vi.fn(),
  };
  const scripts = renderHook(() =>
    useCommunityPluginPublication({
      client,
      localPlugins,
      onPublished: vi.fn(),
      scriptPlugins: true,
    }),
  );
  await waitFor(() => expect(scripts.result.current.optionsLoading).toBe(false));
  expect(scripts.result.current.options.map((option) => option.key)).toEqual([
    "script/pangu",
    "sound/rain",
  ]);
  expect(scripts.result.current.chosenKind).toBe("script");

  const plain = renderHook(() =>
    useCommunityPluginPublication({ client, localPlugins, onPublished: vi.fn() }),
  );
  await waitFor(() => expect(plain.result.current.optionsLoading).toBe(false));
  expect(plain.result.current.options.map((option) => option.key)).toEqual(["sound/rain"]);
});

test("装好脚本插件后打开它的授权页，数据类插件不打开", async () => {
  const installed = pack("script", "pangu");
  const community: CommunityPluginClient = {
    list: vi.fn().mockResolvedValue({ plugins: [item()], has_more: false }),
    detail: vi.fn().mockResolvedValue(item()),
    install: vi.fn().mockResolvedValue(installed),
    rate: vi.fn(),
    delete: vi.fn(),
    packPreview: vi.fn(),
    publish: vi.fn(),
  };
  const onOpenScript = vi.fn();
  const { result } = renderHook(() =>
    useCommunityPluginGallery({ client: community, onOpenScript }),
  );
  await waitFor(() => expect(result.current.plugins).toHaveLength(1));
  await act(async () => {
    await result.current.open(item());
  });
  await act(async () => {
    await result.current.install(true);
  });
  expect(onOpenScript).toHaveBeenCalledExactlyOnceWith(installed);

  (community.install as ReturnType<typeof vi.fn>).mockResolvedValue(pack("sound", "rain"));
  await act(async () => {
    await result.current.install(true);
  });
  expect(onOpenScript).toHaveBeenCalledOnce();

  await act(async () => {
    await result.current.publishDone(item({ owned: true }));
  });
  expect(result.current.actionNotice).toMatch(/已提交审核/);
});

test("旧服务端拒绝脚本插件时说清楚原因", () => {
  expect(communityPluginMessage({ code: "plugin_community_script_unsupported" }, true)).toMatch(
    /服务器暂时还不接受脚本插件/,
  );
});
