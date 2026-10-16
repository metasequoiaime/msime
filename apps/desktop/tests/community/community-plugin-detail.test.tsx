// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommunityPluginDetail } from "../../../../packages/ui/src/community/community-plugin-detail";
import type { CommunityPlugin } from "../../../../packages/ui/src/community/community-plugin-model";

afterEach(cleanup);

const plugin: CommunityPlugin = {
  id: "20000000-0000-4000-8000-000000000001",
  kind: "sound",
  plugin_id: "rain",
  name: "雨声",
  description: "下雨的声音",
  author: "示例作者",
  version: "1.0.0",
  license: "MIT",
  size: 2048,
  sha256: "0".repeat(64),
  downloads: 3,
  rating_count: 1,
  rating_average: 4,
  owned: false,
  my_rating: 0,
  created_at: "2026-09-30T00:00:00Z",
};

const baseProps = () => ({
  plugin,
  actionBusy: false,
  detailBusy: false,
  actionNotice: "",
  error: "",
  signInRequired: false,
  installed: false,
  confirmReplace: false,
  confirmUnpublish: false,
  onLogin: vi.fn(),
  onBack: vi.fn(),
  onInstall: vi.fn(),
  onCancelReplace: vi.fn(),
  onRate: vi.fn(),
  onRequestUnpublish: vi.fn(),
  onUnpublish: vi.fn(),
  onCancelUnpublish: vi.fn(),
  onReport: vi.fn().mockResolvedValue(true),
});

test("renders the plugin summary and install action", () => {
  const props = baseProps();
  render(<CommunityPluginDetail {...props} />);
  expect(screen.getByRole("heading", { name: "雨声" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect(props.onInstall).toHaveBeenCalledWith(false);
});

test("renders owner moderation controls", () => {
  const props = {
    ...baseProps(),
    plugin: { ...plugin, owned: true },
    confirmUnpublish: true,
  };
  render(<CommunityPluginDetail {...props} />);
  expect(screen.getByRole("alertdialog", { name: "确认下架插件" })).not.toBeNull();
});
