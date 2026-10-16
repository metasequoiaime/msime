// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  CommunityResourceDetail,
  type CommunityResourceDetailProps,
  type CommunityResource,
} from "@msime/ui";

afterEach(cleanup);

function resource(): CommunityResource {
  return {
    id: "10000000-0000-4000-8000-000000000001",
    kind: "dictionary",
    name: "合成词库",
    description: "合成说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", code: "he", word: "合成词", weight: 100 }] },
    revision: 7,
    saves: 0,
    saved: false,
    owned: false,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}
function props(initial = resource()): CommunityResourceDetailProps {
  return {
    initial,
    onClose: vi.fn(),
    client: {
      detail: vi.fn(async () => initial),
      save: vi.fn(async () => {}),
      apply: vi.fn(async () => ({ revision: 8, imported: 2, resource_revision: 7 })),
      rate: vi.fn(async () => {}),
      unpublish: vi.fn(async () => {}),
      publish: vi.fn(async () => {}),
    },
  };
}

test("公共详情无需列表能力即可展示词库、按展示版本导入与返回", async () => {
  const options = props();
  render(<CommunityResourceDetail {...options} />);
  expect(screen.getByRole("heading", { name: "合成词库" })).toBeTruthy();
  expect(screen.getByText("合成词")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "导入这版词库到本机" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "导入这版词库到云端" }));
  await waitFor(() => expect(options.client.apply).toHaveBeenCalledWith(options.initial.id, 7));
  await screen.findByText("已导入云端词库，新增或更新 2 个词条。");
  fireEvent.click(screen.getByRole("button", { name: "← 社区" }));
  expect(options.onClose).toHaveBeenCalledOnce();
});

test("公共回复详情按可选能力显示操作，发布新版本后关闭编辑器并刷新", async () => {
  let latest = {
    ...resource(),
    kind: "reply" as const,
    owned: true,
    content: { prompt: "合成提示" },
  };
  const options = props(latest);
  options.client.detail = vi.fn(async () => latest);
  options.client.publish = vi.fn(async () => {
    latest = { ...latest, name: "新版合成回复", revision: 8 };
  });
  render(<CommunityResourceDetail {...options} />);
  expect(screen.getByText("合成提示")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "添加到高情商回复键盘" })).toBeNull();
  expect(screen.queryByRole("button", { name: "从本机高情商回复键盘移除" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "编辑并发布新版本" }));
  expect(screen.getByRole("dialog", { name: "更新社区作品" })).toBeTruthy();
  expect(screen.queryByRole("checkbox", { name: "确认拥有发布内容权利" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "发布新版本" }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "更新社区作品" })).toBeNull());
  await screen.findByRole("heading", { name: "新版合成回复" });
  expect(options.client.publish).toHaveBeenCalledWith(
    options.initial.id,
    "reply",
    "合成词库",
    "合成说明",
    { prompt: "合成提示" },
    7,
  );
  expect(screen.getByText("示例作者 · v8")).toBeTruthy();
});
