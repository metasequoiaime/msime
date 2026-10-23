// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import {
  CandidateRanks,
  Distribution,
} from "../../../../packages/ui/src/settings/statistics-distributions";

afterEach(cleanup);

const slices = [
  { id: "small", title: "合成小类", count: 2, color: "#19a78d", symbol: "小" },
  { id: "zero", title: "合成零类", count: 0, color: "#7772df", symbol: "零" },
  { id: "large", title: "合成大类", count: 6, color: "#4c8ee8", symbol: "大" },
  { id: "unknown", title: "历史未分类", count: 0, color: "#888b92", symbol: "?" },
];

test("环形图保留零值普通图例，隐藏零未知分类，并响应范围更新", () => {
  const { rerender } = render(
    <Distribution title="合成分布" slices={slices} variant="donut" wide footer="合成说明" />,
  );
  const region = screen.getByRole("region", { name: "合成分布" });
  const chart = within(region).getByRole("img", { name: "合成分布环形图" });
  expect(chart.querySelectorAll("[data-donut-segment]")).toHaveLength(2);
  expect(chart.querySelector("strong")?.textContent).toBe("8");
  expect(within(region).getByLabelText("合成小类 2 字符，25.0%")).toBeTruthy();
  expect(within(region).getByLabelText("合成零类 0 字符，0.0%")).toBeTruthy();
  expect(within(region).queryByLabelText(/^历史未分类/)).toBeNull();
  expect(within(region).getByText("合成说明")).toBeTruthy();
  expect(chart.parentElement?.className).toContain("@min-[520px]:grid-cols-");
  rerender(<Distribution title="合成分布" slices={[{ ...slices[3], count: 4 }]} variant="donut" />);
  expect(chart.querySelectorAll("circle[data-donut-segment]")).toHaveLength(1);
  expect(chart.querySelector("strong")?.textContent).toBe("4");
  expect(within(region).getByLabelText("历史未分类 4 字符，100.0%")).toBeTruthy();
  expect(within(region).queryByText("合成说明")).toBeNull();
  expect(chart.parentElement?.className).not.toContain("@min-[520px]:grid-cols-");
});

test("来源排行按计数降序且不修改传入顺序，零值不占行", () => {
  const frozen = Object.freeze(slices.map((slice) => Object.freeze({ ...slice })));
  const input = [...frozen];
  render(<Distribution title="合成来源" slices={input} variant="rank" />);
  const chart = screen.getByRole("img", { name: "合成来源排行" });
  expect(Array.from(chart.children).map((row) => row.getAttribute("aria-label"))).toEqual([
    "合成大类 6 字符，75.0%",
    "合成小类 2 字符，25.0%",
  ]);
  expect(chart.querySelector("i")?.style.width).toBe("100%");
  expect(input.map(({ id }) => id)).toEqual(["small", "zero", "large", "unknown"]);
  expect(screen.queryByRole("img", { name: "合成来源环形图" })).toBeNull();
});

test("空分布保留标题和说明，不绘制图表", () => {
  render(<Distribution title="合成空分布" slices={[]} variant="rank" footer="空范围说明" />);
  const region = screen.getByRole("region", { name: "合成空分布" });
  expect(within(region).getByText("暂无输入记录")).toBeTruthy();
  expect(within(region).queryByRole("img")).toBeNull();
  expect(within(region).getByText("空范围说明")).toBeTruthy();
});

test("候选分布保留九个位置及尾部，排序不随计数改变", () => {
  const { rerender } = render(<CandidateRanks selections={{ ranks: [2, 6], beyond: 2 }} />);
  expect(screen.getByLabelText("首选命中率").textContent).toBe("20.0%");
  expect(screen.getByText("共 10 次上屏")).toBeTruthy();
  const chart = screen.getByRole("img", { name: "候选命中位置分布" });
  expect(Array.from(chart.children).map((row) => row.querySelector("span")?.textContent)).toEqual([
    "第 1 条",
    "第 2 条",
    "第 3 条",
    "第 4 条",
    "第 5 条",
    "第 6 条",
    "第 7 条",
    "第 8 条",
    "第 9 条",
    "第 10 条以后",
  ]);
  expect(within(chart).getByLabelText("第 3 条：0 次，0.0%")).toBeTruthy();
  expect(chart.children[1].querySelector("i")?.style.width).toBe("100%");
  rerender(<CandidateRanks selections={undefined} />);
  expect(screen.getByLabelText("首选命中率").textContent).toBe("—");
  expect(screen.getByText("暂无候选记录。用水杉键盘上屏几次后再回来查看。")).toBeTruthy();
  expect(screen.queryByRole("img", { name: "候选命中位置分布" })).toBeNull();
});
