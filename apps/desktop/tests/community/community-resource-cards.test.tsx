// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { FormEvent } from "react";
import {
  CommunityResourceCard,
  CommunityHarmonyResourceRow,
  type CommunityResource,
  type CommunityResourcePillState,
} from "@msime/ui";

afterEach(cleanup);

function resource(overrides: Partial<CommunityResource> = {}): CommunityResource {
  return {
    id: "synthetic-resource",
    kind: "dictionary",
    name: "合成词库",
    description: "合成说明",
    author: "合成作者",
    content: { entries: [{ kind: "pinyin", code: "he cheng", word: "合成", weight: 1 }] },
    revision: 1,
    saves: 1234,
    saved: false,
    owned: false,
    rating_count: 2,
    rating_average: 4.25,
    my_rating: 0,
    ...overrides,
  };
}

test("resource card exposes its resource and opens once without submitting a parent form", () => {
  const open = vi.fn();
  const submit = vi.fn((event: FormEvent) => event.preventDefault());
  render(
    <form onSubmit={submit}>
      <CommunityResourceCard item={resource()} open={open} />
    </form>,
  );
  const card = screen.getByRole("button", { name: "查看词库 合成词库" });
  expect(within(card).getByText("合成作者")).toBeTruthy();
  expect(within(card).getByText("合成说明")).toBeTruthy();
  expect(within(card).getByText("☆ 4.3 分 · 1,234 人收藏")).toBeTruthy();
  fireEvent.click(card);
  expect(open).toHaveBeenCalledTimes(1);
  expect(submit).not.toHaveBeenCalled();
});

test.each([
  ["dictionary", "字", "共享词条"],
  ["reply", "话", "回复模板"],
] as const)("%s card retains its icon and description fallback", (kind, icon, fallback) => {
  render(
    <CommunityResourceCard
      item={resource({ kind, description: "", rating_count: 0 })}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText(icon).getAttribute("aria-hidden")).toBe("true");
  expect(screen.getByText(fallback)).toBeTruthy();
  expect(screen.getByText("☆ 暂无评分 · 1,234 人收藏")).toBeTruthy();
});

test("resource card marks only the owner's removed work", () => {
  const { rerender } = render(
    <CommunityResourceCard
      item={resource({ owned: true, moderation: "removed" })}
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("我的作品 · 已下架")).toBeTruthy();
  rerender(<CommunityResourceCard item={resource({ moderation: "removed" })} open={vi.fn()} />);
  expect(screen.getByText("合成作者")).toBeTruthy();
  expect(screen.queryByText(/已下架/)).toBeNull();
});

test("Harmony resource row opens from both text and tile while its pill only adds", () => {
  const open = vi.fn();
  const add = vi.fn();
  render(<CommunityHarmonyResourceRow item={resource()} pill="add" open={open} add={add} />);
  expect(screen.getByText("@合成作者 · 1 条")).toBeTruthy();
  expect(screen.queryByText("合成说明")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "查看词库 合成词库" }));
  fireEvent.click(screen.getByText("合", { exact: true }));
  expect(open).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "添加词库 合成词库" }));
  expect(add).toHaveBeenCalledTimes(1);
  expect(open).toHaveBeenCalledTimes(2);
});

test.each<[CommunityResourcePillState, string]>([
  ["adding", "添加中"],
  ["added", "已添加"],
])("Harmony %s pill is disabled while detail remains available", (pill, label) => {
  const open = vi.fn();
  const add = vi.fn();
  render(<CommunityHarmonyResourceRow item={resource()} pill={pill} open={open} add={add} />);
  const button = screen.getByRole("button", { name: `${label}词库 合成词库` }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  button.click();
  expect(add).not.toHaveBeenCalled();
  expect(open).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "查看词库 合成词库" }));
  expect(open).toHaveBeenCalledTimes(1);
});

test("Harmony reply row displays its description and preserves a complete leading grapheme", () => {
  render(
    <CommunityHarmonyResourceRow
      item={resource({ kind: "reply", name: " 👩‍💻 合成模板", author: "", content: {} })}
      pill="add"
      open={vi.fn()}
      add={vi.fn()}
    />,
  );
  expect(screen.getByText("👩‍💻", { exact: true }).getAttribute("aria-hidden")).toBe("true");
  expect(screen.getByText("合成说明")).toBeTruthy();
  expect(screen.queryByText(/@合成作者/)).toBeNull();
});

test("Harmony row uses a placeholder for a blank name and reflects the latest callbacks and pill", () => {
  const first = vi.fn();
  const open = vi.fn();
  const add = vi.fn();
  const item = resource({ kind: "reply", name: " ", author: "", description: "", content: {} });
  const { rerender } = render(
    <CommunityHarmonyResourceRow item={item} pill="adding" open={first} add={first} />,
  );
  expect(screen.getByText("?", { exact: true })).toBeTruthy();
  rerender(<CommunityHarmonyResourceRow item={item} pill="add" open={open} add={add} />);
  fireEvent.click(screen.getByRole("button", { name: "添加回复模板" }));
  fireEvent.click(screen.getByRole("button", { name: "查看回复模板" }));
  expect(add).toHaveBeenCalledTimes(1);
  expect(open).toHaveBeenCalledTimes(1);
  expect(first).not.toHaveBeenCalled();
});
