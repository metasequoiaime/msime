// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  fireEvent.click(screen.getByRole("button", { name: "显示 new-card 的释义" }));
  await act(async () => {
    resolveOld(status({ queue: [{ word: "old-card", phonetic: "", meaning: "old" }] }));
  });
  expect(screen.queryByText("old-card")).toBeNull();
  expect(screen.getByRole("button", { name: "new-card 的释义" })).toBeTruthy();
});

test("a replaced vocabulary client hides a previously revealed card", async () => {
  const previous = client();
  const next = client({
    load: vi.fn(async () =>
      status({ queue: [{ word: "new-card", phonetic: "", meaning: "new meaning" }] }),
    ),
  });
  const view = render(<VocabularyReviewPage client={previous} />);

  fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();

  view.rerender(<VocabularyReviewPage client={next} />);
  await screen.findByText("new-card");
  expect(screen.getByText("点击查看释义")).toBeTruthy();
  expect(screen.queryByText("new meaning")).toBeNull();
});

test.each(["settings", "panel"] as const)(
  "a replaced vocabulary client clears the previous %s state before loading",
  async (surface) => {
    let rejectLoad!: (reason: Error) => void;
    const previous = client();
    const next = client({
      load: vi.fn(() => new Promise<VocabularyReviewStatus>((_, reject) => (rejectLoad = reject))),
    });
    const view = render(<VocabularyReviewPage client={previous} surface={surface} />);
    fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));

    view.rerender(<VocabularyReviewPage client={next} surface={surface} />);
    expect(screen.getByText("正在读取…")).toBeTruthy();
    expect(screen.queryByText("ubiquitous")).toBeNull();
    expect(screen.queryByText("adj. 无处不在的")).toBeNull();
    await act(async () => rejectLoad(new Error("synthetic replacement load failure")));
    expect(screen.queryByRole("button", { name: "认识" })).toBeNull();
    expect(screen.queryByLabelText("词书")).toBeNull();
    expect(screen.queryByRole("button", { name: "清空复习进度" })).toBeNull();

    const recovered = status({
      queue: [{ word: "new-card", phonetic: "", meaning: "new meaning" }],
    });
    vi.mocked(next.load).mockResolvedValueOnce(recovered);
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await screen.findByRole("button", { name: "显示 new-card 的释义" });
    fireEvent.click(screen.getByRole("button", { name: "认识" }));
    expect(next.answer).toHaveBeenCalledWith("cet-4", "new-card", true);
    expect(previous.answer).not.toHaveBeenCalled();
  },
);

test("changing wordbooks hides the next card even when it has the same headword", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const setSettings = vi.fn(
    () => new Promise<VocabularyReviewStatus>((resolve) => (finish = resolve)),
  );
  render(<VocabularyReviewPage client={client({ setSettings })} />);
  fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();

  fireEvent.change(screen.getByLabelText("词书"), { target: { value: "user-list" } });
  expect(setSettings).toHaveBeenCalledOnce();
  await act(async () => {
    finish(status({ settings: { wordbook: "user-list", newPerDay: 20, sessionLimit: 200 } }));
  });
  expect((screen.getByLabelText("词书") as HTMLSelectElement).value).toBe("user-list");
  expect(screen.queryByText("adj. 无处不在的")).toBeNull();
  expect(screen.getByRole("button", { name: "显示 ubiquitous 的释义" })).toBeTruthy();
});

test("revealing while an answer is pending does not reveal the returned queue", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const answer = vi.fn(() => new Promise<VocabularyReviewStatus>((resolve) => (finish = resolve)));
  render(<VocabularyReviewPage client={client({ answer })} />);
  fireEvent.click(await screen.findByRole("button", { name: "认识" }));
  expect(answer).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "显示 ubiquitous 的释义" }));
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();

  await act(async () => {
    finish(status({ queue: [{ word: "ephemeral", phonetic: "", meaning: "adj. 短暂的" }] }));
  });
  expect(screen.getByRole("button", { name: "显示 ephemeral 的释义" })).toBeTruthy();
  expect(screen.queryByText("adj. 短暂的")).toBeNull();
});

test("a failed settings update keeps the current card revealed", async () => {
  const setSettings = vi.fn().mockRejectedValue(new Error("synthetic settings failure"));
  render(<VocabularyReviewPage client={client({ setSettings })} />);
  fireEvent.click(await screen.findByRole("button", { name: "显示 ubiquitous 的释义" }));

  fireEvent.change(screen.getByLabelText("词书"), { target: { value: "user-list" } });
  await screen.findByText("无法读取或保存背单词进度，请稍后重试。已有的进度不会被自动清空。");
  expect(screen.getByText("adj. 无处不在的")).toBeTruthy();
  expect((screen.getByLabelText("词书") as HTMLSelectElement).value).toBe("cet-4");
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
  await waitFor(() => expect(answer).toHaveBeenCalledWith("cet-4", "ubiquitous", true));

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
  await waitFor(() => expect(answer).toHaveBeenCalledWith("cet-4", "ubiquitous", false));
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

test("marks progress reset busy while it is clearing", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const reset = vi.fn(() => new Promise<VocabularyReviewStatus>((resolve) => (finish = resolve)));
  render(<VocabularyReviewPage client={client({ reset })} />);

  fireEvent.click(await screen.findByRole("button", { name: "清空复习进度" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(reset).toHaveBeenCalledOnce());

  const button = screen.getByRole("button", { name: "清空复习进度" });
  expect(button.getAttribute("aria-busy")).toBe("true");
  expect((button as HTMLButtonElement).disabled).toBe(true);
  finish(status());
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

test("marks the import action busy while another vocabulary operation runs", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const reset = vi.fn(() => new Promise<VocabularyReviewStatus>((resolve) => (finish = resolve)));
  render(
    <VocabularyReviewPage
      client={client({ reset, importWordbook: vi.fn(async () => status()) })}
    />,
  );

  const importButton = await screen.findByRole("button", { name: "导入词表文件" });
  fireEvent.click(screen.getByRole("button", { name: "清空复习进度" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(reset).toHaveBeenCalledOnce());

  expect(importButton.getAttribute("aria-busy")).toBe("true");
  expect((importButton as HTMLButtonElement).disabled).toBe(true);
  finish(status());
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

test.each(["reset", "removeWordbook"] as const)(
  "a confirmation from a previous vocabulary generation cannot call %s",
  async (operation) => {
    const result = status({
      settings: { wordbook: "user-list", newPerDay: 20, sessionLimit: 200 },
    });
    const action = vi.fn(async () => result);
    const previous = client({ load: vi.fn(async () => result), [operation]: action });
    const next = client();
    const view = render(<VocabularyReviewPage client={previous} />);
    fireEvent.click(
      await screen.findByRole("button", {
        name: operation === "reset" ? "清空复习进度" : "删除这个词表",
      }),
    );
    await screen.findByRole("alertdialog");
    view.rerender(<VocabularyReviewPage client={next} />);
    await waitFor(() => expect(next.load).toHaveBeenCalledOnce());
    view.rerender(<VocabularyReviewPage client={previous} />);
    await waitFor(() => expect(previous.load).toHaveBeenCalledTimes(2));
    await answerConfirm("confirm");
    expect(action).not.toHaveBeenCalled();
  },
);

test.each(["resolve", "reject"] as const)(
  "a file read from a previous vocabulary generation cannot %s into the current page",
  async (outcome) => {
    let finish!: (value: ArrayBuffer) => void;
    let fail!: (reason: Error) => void;
    const arrayBuffer = vi.fn(
      () =>
        new Promise<ArrayBuffer>((resolve, reject) => {
          finish = resolve;
          fail = reject;
        }),
    );
    const file = new File(["synthetic,合成释义"], "previous.txt");
    Object.defineProperty(file, "arrayBuffer", { value: arrayBuffer });
    const importWordbook = vi.fn(async () => status());
    const previous = client({ importWordbook });
    const next = client({ importWordbook: vi.fn(async () => status()) });
    const view = render(<VocabularyReviewPage client={previous} />);
    await screen.findByRole("button", { name: "导入词表文件" });
    fireEvent.change(view.container.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });
    expect(arrayBuffer).toHaveBeenCalledOnce();
    view.rerender(<VocabularyReviewPage client={next} />);
    await waitFor(() => expect(next.load).toHaveBeenCalledOnce());
    view.rerender(<VocabularyReviewPage client={previous} />);
    await waitFor(() => expect(previous.load).toHaveBeenCalledTimes(2));
    await act(async () => {
      if (outcome === "resolve") finish(new TextEncoder().encode("synthetic,合成释义").buffer);
      else fail(new Error("synthetic read failure"));
    });
    expect(importWordbook).not.toHaveBeenCalled();
    expect(next.importWordbook).not.toHaveBeenCalled();
    expect(screen.queryByText("无法读取所选文件，请重新选择。")).toBeNull();
    expect(screen.queryByText("已导入「previous」。")).toBeNull();
  },
);

test.each(["resolve", "reject"] as const)(
  "a later file choice supersedes an older read that will %s on the same client",
  async (outcome) => {
    let finish!: (value: ArrayBuffer) => void;
    let fail!: (reason: Error) => void;
    const oldBytes = vi.fn(
      () =>
        new Promise<ArrayBuffer>((resolve, reject) => {
          finish = resolve;
          fail = reject;
        }),
    );
    const older = new File(["older,synthetic"], "older.csv");
    Object.defineProperty(older, "arrayBuffer", { value: oldBytes });
    const newer = new File(["newer,synthetic"], "newer.csv");
    Object.defineProperty(newer, "arrayBuffer", {
      value: vi.fn(async () => new TextEncoder().encode("newer,synthetic").buffer),
    });
    const importWordbook = vi.fn(async () => status({ answeredToday: 17 }));
    const view = render(<VocabularyReviewPage client={client({ importWordbook })} />);
    await screen.findByText("今日待复习");
    const input = view.container.querySelector('input[type="file"]')!;
    fireEvent.change(input, { target: { files: [older] } });
    await waitFor(() => expect(oldBytes).toHaveBeenCalledOnce());
    fireEvent.change(input, { target: { files: [newer] } });
    await screen.findByText("已导入「newer」。");
    await act(async () => {
      if (outcome === "resolve") finish(new TextEncoder().encode("older,synthetic").buffer);
      else fail(new Error("synthetic delayed older read failure"));
    });
    expect(importWordbook).toHaveBeenCalledExactlyOnceWith("newer", "newer,synthetic");
    expect(screen.queryByText("无法读取所选文件，请重新选择。")).toBeNull();
    expect(screen.getByText("已导入「newer」。")).toBeTruthy();
  },
);

test("a completed import from a replaced vocabulary client cannot show its success note", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const importWordbook = vi.fn(
    () =>
      new Promise<VocabularyReviewStatus>((resolve) => {
        finish = resolve;
      }),
  );
  const file = new File(["synthetic,合成释义"], "previous.txt");
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new TextEncoder().encode("synthetic,合成释义").buffer,
  });
  const previous = client({ importWordbook });
  const next = client();
  const view = render(<VocabularyReviewPage client={previous} />);
  await screen.findByRole("button", { name: "导入词表文件" });
  fireEvent.change(view.container.querySelector('input[type="file"]')!, {
    target: { files: [file] },
  });
  await waitFor(() => expect(importWordbook).toHaveBeenCalledOnce());
  view.rerender(<VocabularyReviewPage client={next} />);
  await waitFor(() => expect(next.load).toHaveBeenCalledOnce());
  await act(async () => finish(status()));
  expect(screen.queryByText("已导入「previous」。")).toBeNull();
});

test("the current vocabulary client imports decoded text and reports success", async () => {
  const importWordbook = vi.fn(async () => status({ answeredToday: 17 }));
  const file = new File(["synthetic,合成释义"], "current.csv");
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new TextEncoder().encode("synthetic,合成释义").buffer,
  });
  const view = render(<VocabularyReviewPage client={client({ importWordbook })} />);
  await screen.findByRole("button", { name: "导入词表文件" });
  fireEvent.change(view.container.querySelector('input[type="file"]')!, {
    target: { files: [file] },
  });
  expect(await screen.findByText("已导入「current」。")).toBeTruthy();
  expect(importWordbook).toHaveBeenCalledExactlyOnceWith("current", "synthetic,合成释义");
  expect(screen.getByText("17")).toBeTruthy();
});

test("a replaced vocabulary client clears an accepted import notice", async () => {
  const file = new File(["synthetic,合成释义"], "previous.csv");
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new TextEncoder().encode("synthetic,合成释义").buffer,
  });
  const previous = client({ importWordbook: vi.fn(async () => status()) });
  const view = render(<VocabularyReviewPage client={previous} />);
  await screen.findByRole("button", { name: "导入词表文件" });
  fireEvent.change(view.container.querySelector('input[type="file"]')!, {
    target: { files: [file] },
  });
  await screen.findByText("已导入「previous」。");

  const next = client();
  view.rerender(<VocabularyReviewPage client={next} />);
  await waitFor(() => expect(next.load).toHaveBeenCalledOnce());
  await screen.findByRole("button", { name: "显示 ubiquitous 的释义" });
  expect(screen.queryByText("已导入「previous」。")).toBeNull();
});

test("a promise reused by a new vocabulary generation keeps its new request owner", async () => {
  let finish!: (value: VocabularyReviewStatus) => void;
  const shared = new Promise<VocabularyReviewStatus>((resolve) => {
    finish = resolve;
  });
  const previous = client({ load: vi.fn(() => shared) });
  const next = client({ load: vi.fn(() => shared) });
  const view = render(<VocabularyReviewPage client={previous} />);
  await waitFor(() => expect(previous.load).toHaveBeenCalledOnce());
  view.rerender(<VocabularyReviewPage client={next} />);
  await waitFor(() => expect(next.load).toHaveBeenCalledOnce());
  await act(async () => finish(status()));
  expect(await screen.findByText("背单词")).toBeTruthy();
  expect((screen.getByRole("button", { name: "认识" }) as HTMLButtonElement).disabled).toBe(false);
});

test("answers with the book from the displayed queue snapshot after changing books", async () => {
  const api = client({
    setSettings: vi.fn(async (settings) => status({ settings })),
  });
  render(<VocabularyReviewPage client={api} />);
  await screen.findByText("ubiquitous");
  fireEvent.change(screen.getByLabelText("词书"), { target: { value: "user-list" } });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "认识" }).hasAttribute("disabled")).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "认识" }));
  expect(api.answer).toHaveBeenCalledWith("user-list", "ubiquitous", true);
});

function personalStatus(overrides: Partial<VocabularyReviewStatus> = {}): VocabularyReviewStatus {
  return status({
    wordbooks: [
      { id: "personal", name: "生词本", total: 2, builtin: false, personal: true },
      { id: "cet-4", name: "CET-4", total: 4500, builtin: true },
    ],
    settings: { wordbook: "personal", newPerDay: 20, sessionLimit: 200 },
    queue: [{ word: "interview", phonetic: "", meaning: "面试" }],
    ...overrides,
  });
}

test("a personal card can be taken out of the personal wordbook", async () => {
  const removed = personalStatus({ queue: [] });
  const review = client({
    load: vi.fn(async () => personalStatus()),
    removePersonalWord: vi.fn(async () => removed),
  });
  render(<VocabularyReviewPage client={review} surface="panel" />);

  fireEvent.click(await screen.findByRole("button", { name: "把 interview 移出生词本" }));
  await waitFor(() => expect(review.removePersonalWord).toHaveBeenCalledWith("interview"));
  expect(await screen.findByText("今天的复习已经完成。")).toBeTruthy();
});

test("a card from another book offers no personal wordbook action", async () => {
  render(
    <VocabularyReviewPage
      client={client({ removePersonalWord: vi.fn(async () => status()) })}
      surface="panel"
    />,
  );
  await screen.findByText("ubiquitous");
  expect(screen.queryByRole("button", { name: /移出生词本/ })).toBeNull();
});

test("the settings page explains, exports and clears the personal wordbook", async () => {
  const saveExport = vi.fn(async () => "/synthetic/Downloads/生词本.csv");
  const review = client({
    load: vi.fn(async () => personalStatus()),
    removePersonalWord: vi.fn(async () => personalStatus()),
    exportPersonalWordbook: vi.fn(async () => "interview,,面试\n"),
    removeWordbook: vi.fn(async () => status()),
  });
  render(
    <VocabularyReviewPage client={review} openPanel={async () => {}} saveExport={saveExport} />,
  );

  expect(await screen.findByText(/生词本里有 2 个词。/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "导出生词本" }));
  await waitFor(() => expect(saveExport).toHaveBeenCalledWith("生词本.csv", "interview,,面试\n"));
  expect(await screen.findByText("已导出到 /synthetic/Downloads/生词本.csv")).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "清空生词本" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(review.removeWordbook).toHaveBeenCalledWith("personal"));
});

test("a host without the candidate action does not mention the personal wordbook", async () => {
  render(<VocabularyReviewPage client={client()} openPanel={async () => {}} />);
  await screen.findByText("今日待复习");
  expect(screen.queryByText(/加入生词本/)).toBeNull();
});

test("the card offers 朗读 only when the host can speak and the headword is English", async () => {
  const speak = vi.fn(async () => {});
  const view = render(<VocabularyReviewPanel client={client({ speak })} />);
  fireEvent.click(await screen.findByRole("button", { name: "朗读 ubiquitous" }));
  expect(speak).toHaveBeenCalledWith("ubiquitous");
  // 朗读不翻开卡片，也不作答。
  expect(screen.queryByText("adj. 无处不在的")).toBeNull();
  view.unmount();

  render(
    <VocabularyReviewPanel
      client={client({
        speak,
        load: vi.fn(async () =>
          status({ queue: [{ word: "勉強", phonetic: "", meaning: "学习" }] }),
        ),
      })}
    />,
  );
  expect(await screen.findByText("勉強")).toBeTruthy();
  expect(screen.queryByRole("button", { name: /^朗读/ })).toBeNull();
  cleanup();

  render(<VocabularyReviewPanel client={client()} />);
  expect(await screen.findByText("ubiquitous")).toBeTruthy();
  expect(screen.queryByRole("button", { name: /^朗读/ })).toBeNull();
});

test("a voice the host cannot find is reported on the page", async () => {
  const speak = vi.fn(async () => {
    throw new Error("系统没有可用的朗读语音");
  });
  render(<VocabularyReviewPanel client={client({ speak })} />);
  fireEvent.click(await screen.findByRole("button", { name: "朗读 ubiquitous" }));
  expect(await screen.findByText("系统没有可用的朗读语音")).toBeTruthy();
});
