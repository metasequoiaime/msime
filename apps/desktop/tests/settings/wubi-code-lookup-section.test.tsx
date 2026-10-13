// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  SettingsPage,
  WubiCodeLookupSection,
  type DictionaryClient,
  type Snapshot,
  type WubiCodeLookup,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const answer: WubiCodeLookup = {
  phrase: { text: "你好", wubi86: ["wqvb"], wubi98: [] },
  characters: [
    { text: "你", wubi86: ["wq", "wqiy"], wubi98: ["wqiy"] },
    { text: "好", wubi86: ["vbg"], wubi98: ["vbg"] },
  ],
  truncated: false,
};

function rows() {
  return within(screen.getByRole("table", { name: "五笔编码" }))
    .getAllByRole("row")
    .map((row) => row.textContent);
}

test("查码 lists the whole word first, then every character with both profiles", async () => {
  const lookup = vi.fn().mockResolvedValue(answer);
  render(<WubiCodeLookupSection lookup={lookup} profile="wubi98" />);

  const button = screen.getByRole("button", { name: "查码" });
  // 没有文字时不能查。
  expect((button as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("要查五笔编码的文字"), { target: { value: "你好" } });
  fireEvent.click(button);

  await screen.findByRole("table", { name: "五笔编码" });
  expect(lookup).toHaveBeenCalledWith("你好");
  // 当前码表那一列标出来；码表里没有的写成 —。
  expect(rows()).toEqual(["文字86 五笔98 五笔（当前）", "你好wqvb—", "你wq wqiywqiy", "好vbgvbg"]);
});

test("查码 says so when the text has no Han characters, and when the list was cut", async () => {
  const lookup = vi
    .fn()
    .mockResolvedValueOnce({ phrase: null, characters: [], truncated: false })
    .mockResolvedValueOnce({ ...answer, phrase: null, truncated: true });
  render(<WubiCodeLookupSection lookup={lookup} />);
  const input = screen.getByLabelText("要查五笔编码的文字");

  fireEvent.change(input, { target: { value: "hello" } });
  fireEvent.click(screen.getByRole("button", { name: "查码" }));
  await screen.findByText("没有可以查的汉字。");

  fireEvent.change(input, { target: { value: "你好…" } });
  fireEvent.click(screen.getByRole("button", { name: "查码" }));
  await screen.findByText("字太多，只列出了前 2 个不同的字。");
  expect(rows()[0]).toBe("文字86 五笔（当前）98 五笔");
});

test("查码 reports a failure without leaving an old result on screen", async () => {
  const lookup = vi
    .fn()
    .mockResolvedValueOnce(answer)
    .mockRejectedValueOnce(new Error("invalid wubi lookup"));
  render(<WubiCodeLookupSection lookup={lookup} />);
  fireEvent.change(screen.getByLabelText("要查五笔编码的文字"), { target: { value: "你好" } });
  fireEvent.click(screen.getByRole("button", { name: "查码" }));
  await screen.findByRole("table", { name: "五笔编码" });

  fireEvent.click(screen.getByRole("button", { name: "查码" }));
  expect((await screen.findByRole("alert")).textContent).toBe("查码失败，请稍后重试。");
  expect(screen.queryByRole("table", { name: "五笔编码" })).toBeNull();
});

test("editing the lookup text clears the previous result", async () => {
  render(<WubiCodeLookupSection lookup={vi.fn().mockResolvedValue(answer)} />);
  const input = screen.getByLabelText("要查五笔编码的文字");
  fireEvent.change(input, { target: { value: "你好" } });
  fireEvent.click(screen.getByRole("button", { name: "查码" }));
  await screen.findByRole("table", { name: "五笔编码" });

  fireEvent.change(input, { target: { value: "世界" } });
  expect(screen.queryByRole("table", { name: "五笔编码" })).toBeNull();
});

test("an obsolete lookup cannot replace the result for edited text", async () => {
  let finishOld!: (value: WubiCodeLookup) => void;
  const oldRequest = new Promise<WubiCodeLookup>((resolve) => {
    finishOld = resolve;
  });
  const newAnswer: WubiCodeLookup = {
    phrase: null,
    characters: [{ text: "世", wubi86: ["anv"], wubi98: ["anv"] }],
    truncated: false,
  };
  const lookup = vi.fn().mockReturnValueOnce(oldRequest).mockResolvedValueOnce(newAnswer);
  render(<WubiCodeLookupSection lookup={lookup} />);
  const input = screen.getByLabelText("要查五笔编码的文字");
  const button = screen.getByRole("button", { name: "查码" });
  fireEvent.change(input, { target: { value: "你好" } });
  fireEvent.click(button);

  fireEvent.change(input, { target: { value: "世界" } });
  expect((button as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(button);
  await screen.findByRole("table", { name: "五笔编码" });
  expect(rows()[1]).toContain("世");

  await act(async () => finishOld(answer));
  expect(rows()[1]).toContain("世");
  expect(rows().some((row) => row.includes("你好"))).toBe(false);
});

function snapshot(preferences: Partial<Snapshot["preferences"]>): Snapshot {
  return {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  };
}

function dictionary(wubiCodes?: DictionaryClient["wubiCodes"]): DictionaryClient {
  return {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn().mockResolvedValue(undefined),
    ...(wubiCodes ? { wubiCodes } : {}),
  };
}

async function openInputPage(
  preferences: Partial<Snapshot["preferences"]>,
  client: DictionaryClient,
) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot(preferences),
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
        fuzzyPinyin: true,
        dictionary: client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
}

test("the input page shows 五笔查码 with the Wubi settings and asks the host's dictionary", async () => {
  const wubiCodes = vi.fn().mockResolvedValue(answer);
  await openInputPage({ scheme: "wubi", wubi_profile: "wubi86" }, dictionary(wubiCodes));

  const group = screen.getByRole("region", { name: "五笔查码" });
  fireEvent.change(within(group).getByLabelText("要查五笔编码的文字"), {
    target: { value: "你好" },
  });
  fireEvent.click(within(group).getByRole("button", { name: "查码" }));
  await within(group).findByRole("table", { name: "五笔编码" });
  expect(wubiCodes).toHaveBeenCalledWith("你好");
});

test("the input page leaves 五笔查码 out without Wubi or without a host lookup", async () => {
  await openInputPage({ scheme: "quanpin" }, dictionary(vi.fn()));
  expect(screen.queryByRole("region", { name: "五笔查码" })).toBeNull();
  cleanup();

  await openInputPage({ scheme: "wubi" }, dictionary());
  expect(screen.getByRole("switch", { name: "五笔拼音混输" })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "五笔查码" })).toBeNull();
});
