// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommunityCandidateSkinDetail } from "../../../../packages/ui/src/community/community-candidate-skin-detail";
import type { CommunityCandidateSkin } from "../../../../packages/ui/src/community/community-candidate-skin-model";

afterEach(cleanup);

const skin: CommunityCandidateSkin = {
  id: "10000000-0000-4000-8000-000000000001",
  package_id: "forest-window",
  name: "森林候选窗",
  description: "公开说明",
  author: "示例作者",
  version: "1.0.0",
  license: { code: "MIT", assets: "CC0", source: "https://example.invalid" },
  size: 1024,
  file_count: 2,
  downloads: 12,
  rating_count: 2,
  rating_average: 4.5,
  owned: false,
  my_rating: 0,
  created_at: "2026-01-01T00:00:00Z",
  visibility: "public",
  updated_at: "2026-01-01T00:00:00Z",
};

const baseProps = () => ({
  skin,
  load: vi.fn().mockResolvedValue("data:image/png;base64,synthetic"),
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
  onOpenSkinPage: vi.fn(),
  onChangeVisibility: vi.fn(),
  onChangeCategory: vi.fn(),
  onRate: vi.fn(),
  onRequestUnpublish: vi.fn(),
  onUnpublish: vi.fn(),
  onCancelUnpublish: vi.fn(),
  onReport: vi.fn().mockResolvedValue(true),
});

test("renders the candidate skin summary and install action", () => {
  const props = baseProps();
  render(<CommunityCandidateSkinDetail {...props} />);
  expect(screen.getByRole("heading", { name: "森林候选窗" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect(props.onInstall).toHaveBeenCalledWith(false);
});

test("renders the owner visibility and category controls", () => {
  const props = {
    ...baseProps(),
    skin: { ...skin, owned: true, visibility: "private" as const, category: "nature" as const },
  };
  render(<CommunityCandidateSkinDetail {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "公开" }));
  expect(props.onChangeVisibility).toHaveBeenCalledWith("public");
  expect(screen.getByRole("combobox", { name: "修改分类" })).not.toBeNull();
});
