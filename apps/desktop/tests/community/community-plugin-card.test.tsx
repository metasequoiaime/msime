// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { FormEvent } from "react";
import { CommunityPluginCard, type CommunityPlugin } from "@msime/ui";

afterEach(cleanup);

function plugin(overrides: Partial<CommunityPlugin> = {}): CommunityPlugin {
  return {
    id: "synthetic-plugin-publication",
    kind: "sound",
    plugin_id: "rain",
    name: "合成音效",
    description: "合成说明",
    author: "合成作者",
    version: "1.0.0",
    license: "MIT",
    size: 2048,
    sha256: "0".repeat(64),
    downloads: 12345,
    rating_count: 2,
    rating_average: 4.25,
    owned: false,
    my_rating: 0,
    created_at: "2026-09-30T00:00:00Z",
    ...overrides,
  };
}

test("plugin card exposes metadata and opens without submitting a parent form", () => {
  const open = vi.fn();
  const submit = vi.fn((event: FormEvent) => event.preventDefault());
  render(
    <form onSubmit={submit}>
      <CommunityPluginCard plugin={plugin()} open={open} />
    </form>,
  );

  const card = screen.getByRole("button", { name: "查看插件 合成音效" });
  expect(within(card).getByText("音效包 · 合成作者")).toBeTruthy();
  expect(within(card).getByText("合成说明")).toBeTruthy();
  expect(within(card).getByText("↓ 12,345")).toBeTruthy();
  expect(within(card).getByText("☆ 4.3 分")).toBeTruthy();

  fireEvent.click(card);
  expect(open).toHaveBeenCalledOnce();
  expect(submit).not.toHaveBeenCalled();
});

test("plugin card keeps optional description and moderation metadata scoped to the owner", () => {
  const { rerender } = render(
    <CommunityPluginCard plugin={plugin({ owned: true, moderation: "removed" })} open={vi.fn()} />,
  );
  expect(screen.getByText("音效包 · 我的作品 · 已下架")).toBeTruthy();

  rerender(
    <CommunityPluginCard
      plugin={plugin({ description: "", moderation: "removed" })}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("音效包 · 合成作者")).toBeTruthy();
  expect(screen.queryByText("合成说明")).toBeNull();
  expect(screen.queryByText(/我的作品|已下架/)).toBeNull();
});
