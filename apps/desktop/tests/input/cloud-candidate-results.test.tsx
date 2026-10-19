// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  CloudCandidateResults,
  CloudFixedPositions,
  type CloudCandidateResultsProps,
} from "@msime/ui";

const candidate = {
  code: "synthetic",
  canonical_pinyin: "syn'thetic",
  word: "合成候选",
  weight: 10,
};
const item = { context: "synthetic:fixed", code: "syn'thetic", word: "合成候选", position: 2 };
function props(overrides: Partial<CloudCandidateResultsProps> = {}): CloudCandidateResultsProps {
  return {
    kind: "pinyin",
    queryText: "synthetic",
    revision: 7,
    candidates: [candidate],
    busy: false,
    position: 4,
    onRank: vi.fn(),
    onSetFixed: vi.fn(),
    onRemove: vi.fn(),
    ...overrides,
  };
}
afterEach(cleanup);

test("保留候选顺序、显示编码与独立手机提示", () => {
  const second = { code: "synthetic-2", word: "另一候选", weight: 3 };
  const { container } = render(
    <CloudCandidateResults {...props({ candidates: [candidate, second] })} />,
  );
  const list = screen.getByLabelText("云端候选结果");
  expect(list.parentElement).toBe(container);
  expect(within(list).getByText("查询编码：synthetic · 云端版本 7")).toBeTruthy();
  expect(
    within(list)
      .getAllByRole("article")
      .map((row) => row.querySelector("strong")?.textContent),
  ).toEqual(["1. 合成候选", "2. 另一候选"]);
  expect(within(list).getByText("synthetic · 权重 10")).toBeTruthy();
  expect(screen.getByText("点按候选可调频；窄屏下的固定和删除操作会分组显示。").parentElement).toBe(
    container,
  );
});

test("空结果保留查询信息且没有手机操作提示", () => {
  render(<CloudCandidateResults {...props({ candidates: [] })} />);
  expect(screen.getByText("没有匹配的候选")).toBeTruthy();
  expect(screen.getByText("查询编码：synthetic · 云端版本 7")).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
  expect(screen.queryByText(/点按候选可调频/)).toBeNull();
});

test("主按钮和调频传回原候选，固定传回选定位置，删除传回原候选", () => {
  const callbacks = props();
  render(<CloudCandidateResults {...callbacks} />);
  fireEvent.click(screen.getByRole("button", { name: "调频候选 合成候选" }));
  fireEvent.click(screen.getByRole("button", { name: "调频" }));
  fireEvent.click(screen.getByRole("button", { name: "固定" }));
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  expect(callbacks.onRank).toHaveBeenCalledTimes(2);
  expect(vi.mocked(callbacks.onRank).mock.calls[0][0]).toBe(candidate);
  expect(vi.mocked(callbacks.onRank).mock.calls[1][0]).toBe(candidate);
  expect(callbacks.onSetFixed).toHaveBeenCalledExactlyOnceWith(candidate, 4);
  expect(vi.mocked(callbacks.onSetFixed).mock.calls[0][0]).toBe(candidate);
  expect(callbacks.onRemove).toHaveBeenCalledExactlyOnceWith(candidate);
  expect(vi.mocked(callbacks.onRemove).mock.calls[0][0]).toBe(candidate);
});

test("忙碌时禁止全部动作，快捷短语没有动作组且主按钮只读", () => {
  const callbacks = props({ busy: true });
  const { rerender } = render(<CloudCandidateResults {...callbacks} />);
  for (const button of screen.getAllByRole("button")) {
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
  }
  expect(callbacks.onRank).not.toHaveBeenCalled();
  expect(callbacks.onSetFixed).not.toHaveBeenCalled();
  expect(callbacks.onRemove).not.toHaveBeenCalled();
  rerender(<CloudCandidateResults {...callbacks} kind="quick" busy={false} />);
  expect(screen.getAllByRole("button")).toHaveLength(1);
  expect(screen.getByRole("button", { name: "调频候选 合成候选" })).toHaveProperty(
    "disabled",
    true,
  );
  expect(screen.getByRole("article").children).toHaveLength(1);
});

test.each([
  ["pinyin", "𠀀", true],
  ["pinyin", "𠀀甲", false],
  ["english", "x", false],
] as const)("%s 的 %s 删除限制为 %s", (kind, word, disabled) => {
  render(<CloudCandidateResults {...props({ kind, candidates: [{ ...candidate, word }] })} />);
  expect(screen.getByRole("button", { name: "删除" })).toHaveProperty("disabled", disabled);
});

test("固定记录保留静态内容、原对象回调、忙碌禁用和空列表隐藏", () => {
  const onUnfix = vi.fn();
  const { container, rerender } = render(
    <CloudFixedPositions items={[item]} busy={false} onUnfix={onUnfix} />,
  );
  expect(screen.getByLabelText("固定位置").parentElement).toBe(container);
  expect(screen.getByText("此查询的固定位置")).toBeTruthy();
  const row = screen.getByRole("article");
  expect(row.firstElementChild?.tagName).toBe("DIV");
  expect(within(row).getByText("第 2 位 · 合成候选")).toBeTruthy();
  expect(within(row).getByText("syn'thetic")).toBeTruthy();
  expect(within(row).getAllByRole("button")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "取消固定" }));
  expect(onUnfix).toHaveBeenCalledExactlyOnceWith(item);
  expect(onUnfix.mock.calls[0][0]).toBe(item);
  rerender(<CloudFixedPositions items={[item]} busy onUnfix={onUnfix} />);
  expect(screen.getByRole("button", { name: "取消固定" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "取消固定" }));
  expect(onUnfix).toHaveBeenCalledOnce();
  rerender(<CloudFixedPositions items={[]} busy={false} onUnfix={onUnfix} />);
  expect(container.childElementCount).toBe(0);
});
