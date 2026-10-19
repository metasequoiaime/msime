// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { CommunityCandidateSkinCard, type CommunityCandidateSkin } from "@msime/ui";

afterEach(cleanup);

function skin(overrides: Partial<CommunityCandidateSkin> = {}): CommunityCandidateSkin {
  return {
    id: "10000000-0000-4000-8000-000000000001",
    package_id: "synthetic-window",
    name: "合成候选窗",
    description: "合成说明",
    author: "合成作者",
    version: "1.0.0",
    license: { code: "MIT", assets: "CC0", source: "https://example.invalid" },
    size: 1024,
    file_count: 2,
    downloads: 12345,
    rating_count: 2,
    rating_average: 4.25,
    owned: false,
    my_rating: 0,
    created_at: "2026-01-01T00:00:00Z",
    visibility: "public",
    updated_at: "2026-01-01T00:00:00Z",
    category: "nature",
    ...overrides,
  };
}

test("candidate skin card renders its preview metadata and opens the detail", async () => {
  const load = vi.fn().mockResolvedValue("data:image/png;base64,synthetic");
  const open = vi.fn();
  render(<CommunityCandidateSkinCard skin={skin()} load={load} open={open} />);

  const card = screen.getByRole("button", { name: "查看候选窗口皮肤 合成候选窗" });
  expect(within(card).getByText("自然 · 合成作者")).toBeTruthy();
  expect(within(card).getByText("↓ 12,345")).toBeTruthy();
  expect(within(card).getByText("☆ 4.3 分")).toBeTruthy();
  expect(within(card).getByText("素材授权 CC0")).toBeTruthy();
  expect((await within(card).findByAltText("合成候选窗 预览")).getAttribute("src")).toBe(
    "data:image/png;base64,synthetic",
  );

  card.click();
  expect(open).toHaveBeenCalledOnce();
  expect(load).toHaveBeenCalledWith(skin().id, skin().updated_at);
});

test("candidate skin card keeps owner and removed metadata rules", () => {
  const view = render(
    <CommunityCandidateSkinCard
      skin={skin({ owned: true, visibility: "private", moderation: "removed" })}
      load={vi.fn().mockResolvedValue("data:image/png;base64,synthetic")}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("自然 · 我的作品 · 私有 · 已下架")).toBeTruthy();

  view.rerender(
    <CommunityCandidateSkinCard
      skin={skin({ category: undefined, author: "", owned: false, rating_count: 0 })}
      load={vi.fn().mockResolvedValue("data:image/png;base64,synthetic")}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("合成候选窗")).toBeTruthy();
  expect(screen.getByText("☆ 暂无评分")).toBeTruthy();
  expect(screen.queryByText(/我的作品|私有|已下架|自然/)).toBeNull();
});
