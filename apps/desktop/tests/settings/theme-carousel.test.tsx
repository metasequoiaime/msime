// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ThemeCarousel } from "@msime/ui";
import { utilityCss } from "../support/utility-css";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const labels = ["跟随系统", "水杉", "薄荷晨光"];

function renderCarousel(selectedIndex: number) {
  return render(
    <ThemeCarousel labels={labels} selectedIndex={selectedIndex}>
      {labels.map((label) => (
        <article key={label} aria-label={label} />
      ))}
    </ThemeCarousel>,
  );
}

function current() {
  return screen.getAllByRole("button").find((button) => button.getAttribute("aria-current"));
}

test("尺寸变化时按当前卡片重定位，卸载后断开观察", () => {
  const callbacks = new Map<Element, ResizeObserverCallback>();
  const observe = vi.fn();
  const disconnect = vi.fn();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
      }
      callback: ResizeObserverCallback;
      observe(element: Element) {
        callbacks.set(element, this.callback);
        observe(element);
      }
      disconnect = disconnect;
    },
  );
  const { unmount } = renderCarousel(0);
  const track = screen.getByRole("article", { name: labels[0] }).parentElement!.parentElement!;
  Object.defineProperty(track, "clientWidth", { value: 300 });
  track.scrollTo = vi.fn();
  fireEvent.click(screen.getByRole("button", { name: "查看薄荷晨光" }));
  vi.mocked(track.scrollTo).mockClear();

  act(() => callbacks.get(track)!([], {} as ResizeObserver));

  expect(track.scrollTo).toHaveBeenCalledExactlyOnceWith({ left: 600, behavior: "auto" });
  vi.mocked(track.scrollTo).mockClear();
  act(() => callbacks.get(track)!([], {} as ResizeObserver));
  expect(track.scrollTo).not.toHaveBeenCalled();
  expect(callbacks.has(track)).toBe(true);
  unmount();
  expect(disconnect.mock.calls).toHaveLength(observe.mock.calls.length);
});

test("卡片一律拉伸到最高那张的高度：切换、滑动都不改轨道高度，分页条不跳", () => {
  renderCarousel(0);
  const track = screen.getByRole("article", { name: labels[0] }).parentElement!.parentElement!;
  Object.defineProperty(track, "clientWidth", { value: 300 });
  expect(track.style.height).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "下一个主题" }));
  expect(track.style.height).toBe("");
  track.scrollLeft = 600;
  fireEvent.scroll(track);
  expect(track.style.height).toBe("");
  // 高度来自 flex 的拉伸，不是逐张测量：轨道不能是 items-start，slide 要把卡片撑满。
  expect(track.className).not.toContain("items-start");
  expect(track.children[0].className).toContain("[&>article]:flex-1");
  expect(track.parentElement?.className).toContain("theme-carousel-viewport");
});

test("轮播在每张皮肤预览上限制高度，而不是只限制滚动轨道", () => {
  expect(utilityCss("theme-carousel-viewport").replace(/\s+/g, "")).toContain(
    ".theme-carousel-viewport[data-skin-preview]{max-height:520px;overflow:hidden!important;}",
  );
});

test("opens on the selected theme and keeps every card mounted", () => {
  renderCarousel(1);

  expect(screen.getByText("2 / 3")).toBeTruthy();
  expect(current()?.getAttribute("aria-label")).toBe("查看水杉");
  for (const label of labels) expect(screen.getByRole("article", { name: label })).toBeTruthy();
});

test("the arrows step one theme and stop at either end", () => {
  renderCarousel(0);
  const previous = screen.getByRole("button", { name: "上一个主题" }) as HTMLButtonElement;
  const next = screen.getByRole("button", { name: "下一个主题" }) as HTMLButtonElement;

  expect(previous.disabled).toBe(true);
  fireEvent.click(next);
  fireEvent.click(next);
  expect(screen.getByText("3 / 3")).toBeTruthy();
  expect(next.disabled).toBe(true);
  fireEvent.click(previous);
  expect(screen.getByText("2 / 3")).toBeTruthy();
});

test("a dot or an arrow key jumps to that theme", () => {
  renderCarousel(0);

  fireEvent.click(screen.getByRole("button", { name: "查看薄荷晨光" }));
  expect(screen.getByText("3 / 3")).toBeTruthy();
  fireEvent.keyDown(screen.getByRole("button", { name: "上一个主题" }), { key: "ArrowLeft" });
  expect(screen.getByText("2 / 3")).toBeTruthy();
});

test("follows the selection when it changes", () => {
  const { rerender } = renderCarousel(0);

  rerender(
    <ThemeCarousel labels={labels} selectedIndex={2}>
      {labels.map((label) => (
        <article key={label} aria-label={label} />
      ))}
    </ThemeCarousel>,
  );
  expect(screen.getByText("3 / 3")).toBeTruthy();
});

const many = Array.from({ length: 46 }, (_, slide) => `皮肤 ${slide + 1}`);

function renderMany(selectedIndex: number) {
  return render(
    <ThemeCarousel labels={many} selectedIndex={selectedIndex}>
      {many.map((label) => (
        <article key={label} aria-label={label} />
      ))}
    </ThemeCarousel>,
  );
}

function dots() {
  return screen
    .getAllByRole("button")
    .filter((button) => button.getAttribute("aria-label")?.startsWith("查看"));
}

test("with many themes the dots are a window of nine around the current one", () => {
  renderMany(27);

  expect(screen.getByText("28 / 46")).toBeTruthy();
  expect(dots().map((dot) => dot.getAttribute("aria-label"))).toEqual(
    Array.from({ length: 9 }, (_, offset) => `查看皮肤 ${24 + offset}`),
  );
  expect(current()?.getAttribute("aria-label")).toBe("查看皮肤 28");
  for (const label of many) expect(screen.getByRole("article", { name: label })).toBeTruthy();
});

test("the window stays inside the slides at either end and follows the arrows", () => {
  renderMany(0);
  expect(dots()[0].getAttribute("aria-label")).toBe("查看皮肤 1");
  expect(dots()).toHaveLength(9);

  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 9" }));
  expect(screen.getByText("9 / 46")).toBeTruthy();
  expect(dots()[0].getAttribute("aria-label")).toBe("查看皮肤 5");

  fireEvent.click(screen.getByRole("button", { name: "下一个主题" }));
  expect(current()?.getAttribute("aria-label")).toBe("查看皮肤 10");
});

test("the last themes keep a full window", () => {
  renderMany(45);

  expect(dots().map((dot) => dot.getAttribute("aria-label"))).toEqual(
    Array.from({ length: 9 }, (_, offset) => `查看皮肤 ${38 + offset}`),
  );
  expect(current()?.getAttribute("aria-label")).toBe("查看皮肤 46");
});

test("cards added or removed around the viewed one keep it in view; only a new selection moves the carousel", () => {
  const slides = (names: string[], selectedIndex: number) => (
    <ThemeCarousel labels={names} selectedIndex={selectedIndex}>
      {names.map((label) => (
        <article key={label} aria-label={label} />
      ))}
    </ThemeCarousel>
  );
  const { rerender } = render(slides(labels, 0));
  fireEvent.click(screen.getByRole("button", { name: "查看薄荷晨光" }));
  expect(current()?.getAttribute("aria-label")).toBe("查看薄荷晨光");

  // 选中的仍是第一张，但前面插入了一张卡片：停在正在看的那张，不跳回选中的卡片。
  rerender(slides(["新皮肤", ...labels], 1));
  expect(current()?.getAttribute("aria-label")).toBe("查看薄荷晨光");
  expect(screen.getByText("4 / 4")).toBeTruthy();

  // 正在看的卡片被删掉时停在原位置，越界则退到最后一张。
  rerender(slides(["新皮肤", "跟随系统", "水杉"], 1));
  expect(current()?.getAttribute("aria-label")).toBe("查看水杉");
  expect(screen.getByText("3 / 3")).toBeTruthy();

  rerender(slides(["新皮肤", "跟随系统", "水杉"], 0));
  expect(current()?.getAttribute("aria-label")).toBe("查看新皮肤");
});

test("「回到当前」把翻走的轮播带回在用的那张，停在那张时不可点", () => {
  renderMany(27);
  const back = screen.getByRole("button", { name: "回到当前主题" }) as HTMLButtonElement;
  expect(back.textContent).toBe("回到当前");
  expect(back.disabled).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "下一个主题" }));
  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 32" }));
  expect(screen.getByText("32 / 46")).toBeTruthy();
  expect(back.disabled).toBe(false);

  fireEvent.click(back);
  expect(screen.getByText("28 / 46")).toBeTruthy();
  expect(current()?.getAttribute("aria-label")).toBe("查看皮肤 28");
  expect(back.disabled).toBe(true);
});

test("「回到当前」平滑滚到选中卡片的位置", () => {
  renderCarousel(1);
  const track = screen.getByRole("article", { name: labels[0] }).parentElement!.parentElement!;
  Object.defineProperty(track, "clientWidth", { value: 300 });
  track.scrollTo = vi.fn();
  fireEvent.click(screen.getByRole("button", { name: "查看薄荷晨光" }));
  vi.mocked(track.scrollTo).mockClear();

  fireEvent.click(screen.getByRole("button", { name: "回到当前主题" }));
  expect(track.scrollTo).toHaveBeenCalledExactlyOnceWith({ left: 300, behavior: "smooth" });
  expect(screen.getByText("2 / 3")).toBeTruthy();
});
