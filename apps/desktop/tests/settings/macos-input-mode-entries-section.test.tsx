// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  type EditionInfo,
  MacosInputModeEntriesSection,
  macosInputModeEntries,
  macosInputModeEntriesFor,
  type MacosInputModesClient,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const prefix = "app.msime.inputmethod.MetasequoiaIME.";
const allSchemes = [
  "quanpin",
  "shuangpin",
  "wubi",
  "japanese",
  "korean",
  "cantonese",
  "zhuyin",
  "vietnamese",
  "tibetan",
  "stroke",
] as const;

/** 展开后还没加入的入口，按语言分组：`[语言, [入口名…]]`。 */
async function missingGroups() {
  const toggle = await screen.findByRole("button", { name: /查看怎么添加和全部/ });
  fireEvent.click(toggle);
  const list = await screen.findByRole("list", { name: "还没加入的入口" });
  return within(list)
    .getAllByRole("listitem")
    .map((item) => {
      const [language, names] = [...item.children];
      return [language.textContent, [...names.children].map((name) => name.textContent)] as const;
    });
}

function client(enabled: string[] | null, openSettings = vi.fn(() => Promise.resolve())) {
  return {
    enabled: vi.fn(() => Promise.resolve(enabled)),
    openSettings,
  } satisfies MacosInputModesClient;
}

test("names the language each missing entry sits under in the add dialog", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client(
        ["Hans", "Shuangpin", "Wubi", "Roman", "Japanese", "Korean", "Vietnamese"].map(
          (mode) => prefix + mode,
        ),
      )}
      scheme="quanpin"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  expect(await missingGroups()).toEqual([
    ["粤语", ["水杉输入法 · 粤"]],
    ["繁体中文", ["水杉输入法 · 注"]],
    ["藏语", ["水杉输入法 · 藏"]],
    ["简体中文", ["水杉输入法 · 笔"]],
  ]);
  expect(screen.getByText(/还有 4 个入口没加进输入法菜单/)).toBeTruthy();
  expect(screen.queryByText(/菜单栏里还没有/)).toBeNull();
  expect(screen.getByRole("button", { name: "打开键盘设置" })).toBeTruthy();
});

test("keeps the instructions and the full list folded until asked for", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans"])}
      scheme="quanpin"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  const toggle = await screen.findByRole("button", { name: /查看怎么添加和全部 10 个入口/ });
  expect(toggle.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("list", { name: "还没加入的入口" })).toBeNull();
  expect(screen.getByText(/左下角「\+」/).closest("[hidden]")).not.toBeNull();

  fireEvent.click(toggle);
  expect(toggle.getAttribute("aria-expanded")).toBe("true");
  expect(screen.getByText(/左下角「\+」/).closest("[hidden]")).toBeNull();
  expect(screen.getByRole("list", { name: "还没加入的入口" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: /收起/ }));
  expect(screen.queryByRole("list", { name: "还没加入的入口" })).toBeNull();
});

test("leads with the current scheme's entry when it is missing", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans"])}
      scheme="cantonese"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  const text = (await screen.findByText(/菜单栏里还没有/)).textContent ?? "";
  expect(text.startsWith("菜单栏里还没有「水杉输入法 · 粤」，到「粤语」下添加")).toBe(true);
  expect(text).not.toContain("重新登录");
  // 简体中文下的几个入口排在同一行，不再各写一遍语言。
  expect((await missingGroups())[0]).toEqual([
    "简体中文",
    ["水杉输入法 · 双", "水杉输入法 · 五", "水杉输入法 · 笔", "水杉输入法 · 英"],
  ]);
});

test("leads with the Stroke entry when Stroke is the current scheme", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans"])}
      scheme="stroke"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  const text = (await screen.findByText(/菜单栏里还没有/)).textContent ?? "";
  expect(text.startsWith("菜单栏里还没有「水杉输入法 · 笔」")).toBe(true);
});

test("leaves out the entries of schemes the host does not offer", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client([prefix + "Hans", prefix + "Roman"])}
      scheme="quanpin"
      inputSchemes={["quanpin"]}
      onError={vi.fn()}
    />,
  );

  expect(await screen.findByText("输入法菜单里已经有水杉输入法的全部入口。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("shows nothing when the input source list cannot be read", async () => {
  const modes = client(null);
  const { container } = render(
    <MacosInputModeEntriesSection
      client={modes}
      scheme="cantonese"
      inputSchemes={allSchemes}
      onError={vi.fn()}
    />,
  );

  await vi.waitFor(() => expect(modes.enabled).toHaveBeenCalled());
  expect(container.textContent).toBe("");
});

test("reads the list again while the user is in System Settings", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const enabled = vi
    .fn<() => Promise<string[] | null>>()
    .mockResolvedValueOnce([prefix + "Hans"])
    .mockResolvedValue([prefix + "Hans", prefix + "Cantonese"]);
  render(
    <MacosInputModeEntriesSection
      client={{ enabled, openSettings: () => Promise.resolve() }}
      scheme="cantonese"
      inputSchemes={["quanpin", "cantonese"]}
      onError={vi.fn()}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(3000);
  });
  await vi.waitFor(() => expect(screen.queryByText(/菜单栏里还没有/)).toBeNull());
  expect(await missingGroups()).toEqual([["简体中文", ["水杉输入法 · 英"]]]);
});

test("reports a failure when opening keyboard settings fails", async () => {
  const onError = vi.fn();
  render(
    <MacosInputModeEntriesSection
      client={client(
        [prefix + "Hans"],
        vi.fn(() => Promise.reject(new Error("unavailable"))),
      )}
      scheme="quanpin"
      inputSchemes={allSchemes}
      onError={onError}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  await vi.waitFor(() =>
    expect(onError).toHaveBeenCalledWith(
      "无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。",
    ),
  );
});

const wubiEdition: EditionInfo = {
  id: "wubi",
  display_name: "水杉五笔",
  input_schemes: ["wubi"],
  default_scheme: "wubi",
  temporary_japanese: false,
  neural_keyboard: false,
  offline_glosses: true,
  handwriting: true,
  wubi_mixed_pinyin_default: true,
};

test("full keeps the table and each edition lists only its own entries under its own name", () => {
  expect(macosInputModeEntriesFor()).toBe(macosInputModeEntries);
  // 五笔版的主模式就是「五」：bundle 只声明 Hans（用五笔的字）和 Roman，见 edition_bundle.py。
  expect(macosInputModeEntriesFor(wubiEdition).map(({ mode, name }) => [mode, name])).toEqual([
    ["Hans", "水杉五笔 · 五"],
    ["Roman", "水杉五笔 · 英"],
  ]);
  expect(
    macosInputModeEntriesFor({
      ...wubiEdition,
      id: "pinyin",
      display_name: "水杉拼音",
      input_schemes: ["quanpin", "shuangpin"],
      default_scheme: "quanpin",
    }).map(({ mode, name }) => [mode, name]),
  ).toEqual([
    ["Hans", "水杉拼音 · 中"],
    ["Shuangpin", "水杉拼音 · 双"],
    ["Roman", "水杉拼音 · 英"],
  ]);
  // 日文、越南文、藏文版的主模式是本版本方案的字，主模式和「英」都登记在这个方案的语言下；五笔版仍在「简体中文」下。
  for (const [id, display_name, mark, language] of [
    ["japanese", "水杉日语", "日", "日语"],
    ["vietnamese", "水杉越南语", "越", "越南语"],
    ["tibetan", "水杉藏文", "藏", "藏语"],
  ] as const) {
    expect(
      macosInputModeEntriesFor({
        ...wubiEdition,
        id,
        display_name,
        input_schemes: [id],
        default_scheme: id,
        offline_glosses: false,
        handwriting: false,
        wubi_mixed_pinyin_default: false,
      }).map((entry) => [entry.mode, entry.name, entry.language]),
    ).toEqual([
      ["Hans", `${display_name} · ${mark}`, language],
      ["Roman", `${display_name} · 英`, language],
    ]);
  }
  expect(macosInputModeEntriesFor(wubiEdition).map(({ language }) => language)).toEqual([
    "简体中文",
    "简体中文",
  ]);
});

test("an edition whose entries are all in the list says so under its own name", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client(["app.msime.inputmethod.wubi.Hans", "app.msime.inputmethod.wubi.Roman"])}
      scheme="wubi"
      inputSchemes={["wubi"]}
      edition={wubiEdition}
      onError={vi.fn()}
    />,
  );
  expect(await screen.findByText("输入法菜单里已经有水杉五笔的全部入口。")).toBeTruthy();
});

test("an edition names its missing entry by the edition's name", async () => {
  render(
    <MacosInputModeEntriesSection
      client={client(["app.msime.inputmethod.wubi.Roman"])}
      scheme="wubi"
      inputSchemes={["wubi"]}
      edition={wubiEdition}
      onError={vi.fn()}
    />,
  );
  expect(await missingGroups()).toEqual([["简体中文", ["水杉五笔 · 五"]]]);
  expect(screen.queryByText(/水杉输入法/)).toBeNull();
});

test("更换客户端后先隐藏上一份入口列表，等待当前客户端回答", async () => {
  let resolve!: (value: string[] | null) => void;
  const next = client(null);
  next.enabled.mockImplementation(() => new Promise((yes) => (resolve = yes)));
  const props = {
    scheme: "quanpin" as const,
    inputSchemes: ["quanpin"] as const,
    onError: vi.fn(),
  };
  const view = render(
    <MacosInputModeEntriesSection
      {...props}
      client={client([prefix + "Hans", prefix + "Roman"])}
    />,
  );
  expect(await screen.findByText("输入法菜单里已经有水杉输入法的全部入口。")).toBeTruthy();
  view.rerender(<MacosInputModeEntriesSection {...props} client={next} />);
  expect(screen.queryByText("输入法菜单里已经有水杉输入法的全部入口。")).toBeNull();
  await act(async () => resolve([prefix + "Hans"]));
  expect(await screen.findByText(/还有 1 个入口/)).toBeTruthy();
});

test("更换客户端后停止上一份打开系统设置的轮询窗口", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const previous = client([prefix + "Hans"]);
  const current = client([prefix + "Hans"]);
  const props = {
    scheme: "quanpin" as const,
    inputSchemes: ["quanpin"] as const,
    onError: vi.fn(),
  };
  const view = render(<MacosInputModeEntriesSection {...props} client={previous} />);
  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  view.rerender(<MacosInputModeEntriesSection {...props} client={current} />);
  await vi.waitFor(() => expect(current.enabled).toHaveBeenCalledTimes(1));
  await act(async () => vi.advanceTimersByTimeAsync(3000));
  expect(current.enabled).toHaveBeenCalledTimes(1);
});

test("卸载后打开系统设置的旧失败不再调用展示错误回调", async () => {
  let reject!: (error: unknown) => void;
  const onError = vi.fn();
  const view = render(
    <MacosInputModeEntriesSection
      client={client(
        [prefix + "Hans"],
        vi.fn(() => new Promise<void>((_, no) => (reject = no))),
      )}
      scheme="quanpin"
      inputSchemes={["quanpin"]}
      onError={onError}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "打开键盘设置" }));
  view.unmount();
  await act(async () => reject(new Error("合成旧窗口错误")));
  expect(onError).not.toHaveBeenCalled();
});
