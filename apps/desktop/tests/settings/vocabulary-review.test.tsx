// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { answerConfirm } from "../support/confirm";
import {
  VocabularyReviewPage,
  VocabularyReviewPanel,
  type VocabularyReviewClient,
  type VocabularyReviewStatus,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function status(overrides: Partial<VocabularyReviewStatus> = {}): VocabularyReviewStatus {
  return {
    wordbooks: [
      { id: "cet-4", name: "CET-4", total: 4500, builtin: true },
      { id: "user-list", name: "我的词表", total: 12, builtin: false },
    ],
    settings: { wordbook: "cet-4", newPerDay: 20, sessionLimit: 200 },
    due: 24,
    answeredToday: 6,
    introducing: 2,
    remaining: 4474,
    queue: [
      { word: "ubiquitous", phonetic: "/juːˈbɪkwɪtəs/", meaning: "adj. 无处不在的" },
      { word: "ephemeral", phonetic: "", meaning: "adj. 短暂的" },
    ],
    ...overrides,
  };
}

function client(overrides: Partial<VocabularyReviewClient> = {}): VocabularyReviewClient {
  return {
    load: vi.fn(async () => status()),
    answer: vi.fn(async () => status()),
    setSettings: vi.fn(async () => status()),
    reset: vi.fn(async () => status({ due: 0, answeredToday: 0, queue: [] })),
    ...overrides,
  };
}

test("shows today's counts and the first card with its meaning hidden", async () => {
  render(<VocabularyReviewPage client={client()} />);

  expect(await screen.findByText("今日待复习")).toBeTruthy();
  expect(screen.getByText("24")).toBeTruthy();
  expect(screen.getByText("已完成")).toBeTruthy();
  expect(screen.getByText("6")).toBeTruthy();

  expect(screen.getByText("ubiquitous")).toBeTruthy();
  expect(screen.getByText("/juːˈbɪkwɪtəs/")).toBeTruthy();
  // The whole point of a flashcard is that the answer is not on screen yet.
  expect(screen.queryByText("adj. 无处不在的")).toBeNull();
  expect(screen.getByText("点击查看释义")).toBeTruthy();
});

test("a replaced vocabulary client ignores its older load response", async () => {
  let resolveOld!: (value: VocabularyReviewStatus) => void;
  const old = client({
    load: vi.fn(() => new Promise<VocabularyReviewStatus>((resolve) => (resolveOld = resolve))),
  });
  const nextStatus = status({ queue: [{ word: "new-card", phonetic: "", meaning: "new" }] });
  const next = client({ load: vi.fn().mockResolvedValue(nextStatus) });
  const view = render(<VocabularyReviewPage client={old} />);
  await waitFor(() => expect(old.load).toHaveBeenCalled());
  view.rerender(<VocabularyReviewPage client={next} />);
  await screen.findByText("new-card");
  resolveOld(status({ queue: [{ word: "old-card", phonetic: "", meaning: "old" }] }));
  await Promise.resolve();
  expect(screen.queryByText("old-card")).toBeNull();
});

test("tapping the card reveals the meaning", async () => {
  render(<VocabularyReviewPage client={client()} />);

  fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();
  expect(screen.queryByText("点击查看释义")).toBeNull();
});

test("answering sends the grade and hides the next card's meaning again", async () => {
  const answer = vi.fn(async () =>
    status({ queue: [{ word: "ephemeral", phonetic: "", meaning: "adj. 短暂的" }] }),
  );
  render(<VocabularyReviewPage client={client({ answer })} />);

  fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "认识" }));
  await waitFor(() => expect(answer).toHaveBeenCalledWith("ubiquitous", true));

  // A revealed card must not hand its revealed state to the next one, or the second card of every
  // session shows its answer before the user has thought about it.
  await screen.findByText("ephemeral");
  expect(screen.getByText("点击查看释义")).toBeTruthy();
  expect(screen.queryByText("adj. 短暂的")).toBeNull();
});

test("不认识 is sent as a failed grade", async () => {
  const answer = vi.fn(async () => status());
  render(<VocabularyReviewPage client={client({ answer })} />);

  fireEvent.click(await screen.findByRole("button", { name: "不认识" }));
  await waitFor(() => expect(answer).toHaveBeenCalledWith("ubiquitous", false));
});

test("only one request is in flight, so a double tap grades once", async () => {
  let release: (value: VocabularyReviewStatus) => void = () => {};
  const answer = vi.fn(
    () =>
      new Promise<VocabularyReviewStatus>((resolve) => {
        release = resolve;
      }),
  );
  render(<VocabularyReviewPage client={client({ answer })} />);

  const known = await screen.findByRole("button", { name: "认识" });
  fireEvent.click(known);
  expect(known.getAttribute("aria-busy")).toBe("true");
  expect((known as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(known);
  expect(answer).toHaveBeenCalledTimes(1);
  release(status());
  await waitFor(() => expect(screen.getByText("ubiquitous")).toBeTruthy());
});

test("a late review response is ignored after the panel unmounts", async () => {
  let release: (value: VocabularyReviewStatus) => void = () => {};
  const answer = vi.fn(
    () =>
      new Promise<VocabularyReviewStatus>((resolve) => {
        release = resolve;
      }),
  );
  const view = render(<VocabularyReviewPage client={client({ answer })} />);
  fireEvent.click(await screen.findByRole("button", { name: "认识" }));
  view.unmount();
  release(status());
  await Promise.resolve();
});

test("choosing a wordbook saves it", async () => {
  const setSettings = vi.fn(async () => status());
  render(<VocabularyReviewPage client={client({ setSettings })} />);

  fireEvent.change(await screen.findByLabelText("词书"), { target: { value: "user-list" } });
  await waitFor(() =>
    expect(setSettings).toHaveBeenCalledWith({
      wordbook: "user-list",
      newPerDay: 20,
      sessionLimit: 200,
    }),
  );
});

test("the daily new-word count saves and rejects a negative value", async () => {
  const setSettings = vi.fn(async () => status());
  render(<VocabularyReviewPage client={client({ setSettings })} />);

  const input = await screen.findByLabelText("每日新词");
  fireEvent.change(input, { target: { value: "5" } });
  await waitFor(() =>
    expect(setSettings).toHaveBeenCalledWith({
      wordbook: "cet-4",
      newPerDay: 5,
      sessionLimit: 200,
    }),
  );

  setSettings.mockClear();
  fireEvent.change(input, { target: { value: "-3" } });
  expect(setSettings).not.toHaveBeenCalled();
});

test("clearing the progress asks first and does nothing when cancelled", async () => {
  const reset = vi.fn(async () => status({ due: 0, answeredToday: 0, queue: [] }));
  render(<VocabularyReviewPage client={client({ reset })} />);

  fireEvent.click(await screen.findByRole("button", { name: "清空复习进度" }));
  await answerConfirm("cancel");
  expect(reset).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "清空复习进度" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(reset).toHaveBeenCalled());
  expect(await screen.findByText("今天的复习已经完成。")).toBeTruthy();
});

test("a host that cannot import a file renders no import button", async () => {
  render(<VocabularyReviewPage client={client()} />);
  await screen.findByText("背单词");
  // Rendering nothing beats a button that fails when pressed.
  expect(screen.queryByRole("button", { name: "导入词表文件" })).toBeNull();

  cleanup();
  render(<VocabularyReviewPage client={client({ importWordbook: vi.fn(async () => status()) })} />);
  expect(await screen.findByRole("button", { name: "导入词表文件" })).toBeTruthy();
});

test("a word list over 1 MiB is refused with a message instead of failing silently", async () => {
  const importWordbook = vi.fn(async () => status());
  const { container } = render(<VocabularyReviewPage client={client({ importWordbook })} />);
  await screen.findByRole("button", { name: "导入词表文件" });
  const input = container.querySelector('input[type="file"]') as HTMLInputElement;
  const file = new File(["单词"], "big.txt", { type: "text/plain" });
  Object.defineProperty(file, "size", { value: 1_048_577 });
  fireEvent.change(input, { target: { files: [file] } });
  expect(await screen.findByText("文件不能超过 1 MB，请拆分后分别导入。")).toBeTruthy();
  expect(importWordbook).not.toHaveBeenCalled();
});

test("a bundled wordbook offers no delete, an imported one does", async () => {
  const removeWordbook = vi.fn(async () => status());
  render(<VocabularyReviewPage client={client({ removeWordbook })} />);

  await screen.findByText("背单词");
  expect(screen.queryByRole("button", { name: "删除这个词表" })).toBeNull();

  cleanup();
  const imported = client({
    removeWordbook,
    load: vi.fn(async () =>
      status({ settings: { wordbook: "user-list", newPerDay: 20, sessionLimit: 200 } }),
    ),
  });
  render(<VocabularyReviewPage client={imported} />);
  fireEvent.click(await screen.findByRole("button", { name: "删除这个词表" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(removeWordbook).toHaveBeenCalledWith("user-list"));
});

test("marks wordbook deletion busy while it is being removed", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const removeWordbook = vi.fn(
    () => new Promise<VocabularyReviewStatus>((resolve) => (finish = resolve)),
  );
  const imported = client({
    removeWordbook,
    load: vi.fn(async () =>
      status({ settings: { wordbook: "user-list", newPerDay: 20, sessionLimit: 200 } }),
    ),
  });
  render(<VocabularyReviewPage client={imported} />);

  fireEvent.click(await screen.findByRole("button", { name: "删除这个词表" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(removeWordbook).toHaveBeenCalledWith("user-list"));

  const button = screen.getByRole("button", { name: "删除这个词表" });
  expect(button.getAttribute("aria-busy")).toBe("true");
  expect((button as HTMLButtonElement).disabled).toBe(true);
  finish(status());
});

test("no wordbook selected asks for one instead of showing an empty card", async () => {
  const empty = client({
    load: vi.fn(async () =>
      status({ settings: { wordbook: "", newPerDay: 20, sessionLimit: 200 }, queue: [] }),
    ),
  });
  render(<VocabularyReviewPage client={empty} />);

  expect(await screen.findByText("先选一本词书。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "认识" })).toBeNull();
});

test("a failed read keeps the existing progress and says so", async () => {
  const failing = client({ load: vi.fn(async () => Promise.reject(new Error("storage"))) });
  render(<VocabularyReviewPage client={failing} />);

  expect(
    await screen.findByText("无法读取或保存背单词进度，请稍后重试。已有的进度不会被自动清空。"),
  ).toBeTruthy();
});

test("a host with a panel keeps the settings page for managing, not for studying", async () => {
  const openPanel = vi.fn(async () => {});
  render(<VocabularyReviewPage client={client()} openPanel={openPanel} />);

  // 设置页是抽屉，不是目的地：它管词书，不发卡片。
  expect(await screen.findByLabelText("词书")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "认识" })).toBeNull();
  expect(screen.queryByText("ubiquitous")).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "打开背单词面板" }));
  await waitFor(() => expect(openPanel).toHaveBeenCalled());
});

test("the panel deals cards and offers nothing to manage", async () => {
  render(<VocabularyReviewPanel client={client()} />);

  expect(await screen.findByText("ubiquitous")).toBeTruthy();
  expect(screen.getByRole("button", { name: "认识" })).toBeTruthy();
  // 词书管理和清空进度留在设置页，面板只做复习这一件事。
  expect(screen.queryByLabelText("词书")).toBeNull();
  expect(screen.queryByRole("button", { name: "清空复习进度" })).toBeNull();
  expect(screen.queryByRole("button", { name: "导入词表文件" })).toBeNull();
});

test("a host without panels keeps the review on the page rather than losing it", async () => {
  // 没有面板可去时，设置页必须继续发卡，否则用户有词书却无处可背。
  render(<VocabularyReviewPage client={client()} />);
  expect(await screen.findByText("ubiquitous")).toBeTruthy();
  expect(screen.getByRole("button", { name: "认识" })).toBeTruthy();
  expect(screen.getByLabelText("词书")).toBeTruthy();
});

test("a wordbook from a plugin is marked and cannot be deleted here", async () => {
  const removeWordbook = vi.fn(async () => status());
  const fromPack = client({
    removeWordbook,
    load: vi.fn(async () =>
      status({
        wordbooks: [
          { id: "cet-4", name: "CET-4", total: 4500, builtin: true },
          { id: "pack-cs-words", name: "计算机词汇", total: 300, builtin: false, pack: true },
        ],
        settings: { wordbook: "pack-cs-words", newPerDay: 20, sessionLimit: 200 },
      }),
    ),
  });
  render(<VocabularyReviewPage client={fromPack} />);
  expect(await screen.findByRole("option", { name: "计算机词汇（300 词） · 插件" })).toBeTruthy();
  expect(screen.getByText(/这本书来自插件，在「插件」页卸载/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "删除这个词表" })).toBeNull();
});
