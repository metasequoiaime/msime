// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommunitySkinDetail } from "../../../../packages/ui/src/community/community-skin-detail";
import type { CommunitySkin } from "../../../../packages/ui/src/community/community-skin-model";
import { defaultTouchKeyboardSkinDesign as design } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

const skin: CommunitySkin = {
  id: "10000000-0000-4000-8000-000000000001",
  name: "示例皮肤",
  description: "公开说明",
  author: "示例作者",
  design,
  downloads: 12,
  rating_count: 2,
  rating_average: 4.5,
  owned: false,
  my_rating: 0,
};

const baseProps = () => ({
  skin,
  theme: "light" as const,
  actionBusy: false,
  detailBusy: false,
  actionNotice: "",
  confirmUnpublish: false,
  trial: null,
  error: "",
  signInRequired: false,
  onBack: vi.fn(),
  onDownload: vi.fn(),
  onFinishTrial: vi.fn(),
  onChangeCategory: vi.fn(),
  onRate: vi.fn(),
  onRequestUnpublish: vi.fn(),
  onUnpublish: vi.fn(),
  onCancelUnpublish: vi.fn(),
  onReport: vi.fn().mockResolvedValue(true),
});

test("renders the skin summary and download action", () => {
  const props = baseProps();
  render(<CommunitySkinDetail {...props} />);
  expect(screen.getByRole("heading", { name: "示例皮肤" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下载并试用" }));
  expect(props.onDownload).toHaveBeenCalledOnce();
});

test("renders trial controls and reports the selected keep action", () => {
  const props = { ...baseProps(), trial: { id: "trial-1", name: "示例皮肤" } };
  render(<CommunitySkinDetail {...props} />);
  expect(screen.getByText("正在试用：示例皮肤")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "保留使用" }));
  expect(props.onFinishTrial).toHaveBeenCalledWith(true);
});

test("keeps owner moderation controls and hides public rating controls", () => {
  const props = { ...baseProps(), skin: { ...skin, owned: true, category: "nature" as const } };
  render(<CommunitySkinDetail {...props} />);
  expect(screen.queryByRole("button", { name: "评 4 星" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下架这款皮肤" }));
  expect(props.onRequestUnpublish).toHaveBeenCalledOnce();
});
