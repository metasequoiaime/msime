// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  PluginsSection,
  defaultPluginPreferences,
  draftIssue,
  emptyDraft,
  packFromDraft,
  parsePhraseLines,
  parseWordLines,
  type AuthoredPack,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  type PluginPreferences,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base = {
  version: "1.0.0",
  license: "CC0-1.0",
  author: null,
  description: null,
  builtin: false,
};

const userPhrases: PluginPackage = {
  ...base,
  id: "shared-phrases",
  kind: "phrase_table",
  name: "社区短语",
  author: "别人",
  license: "CC-BY-4.0",
  phrases: [{ key: "hi", text: "你好" }],
};
const officialPhrases: PluginPackage = {
  ...base,
  id: "latex-symbols",
  kind: "phrase_table",
  name: "LaTeX 符号",
  builtin: true,
  phrases: [{ key: "alpha", text: "α" }],
};

/** 一个带编辑器的宿主：保存后目录里多出装好的包，与真实宿主一样。 */
function editorClient(packages: PluginPackage[] = [], overrides: Partial<PluginClient> = {}) {
  let installed = [...packages];
  const catalog = vi.fn(async (): Promise<PluginCatalogResult> => ({
    packages: installed,
    issues: [],
  }));
  const saveAuthoredPack = vi.fn(async (pack: AuthoredPack, _replace: boolean) => {
    const saved: PluginPackage = {
      ...base,
      id: pack.id ?? "my-0a1b2c3d",
      kind: pack.kind,
      name: pack.name,
      version: pack.version,
      license: pack.license,
      author: pack.author,
      description: pack.description,
      phrases: pack.phrases,
      commands: pack.commands,
      groups: pack.groups?.map((group) => ({ ...group, keywords: group.keywords ?? "" })),
      word_count: pack.words?.length,
      first_words: pack.words?.slice(0, 5).map((entry) => entry.word),
    };
    installed = [...installed.filter((item) => item.id !== saved.id), saved];
    return saved;
  });
  const client: PluginClient = {
    catalog,
    importPack: vi.fn(async () => null),
    remove: vi.fn(async () => undefined),
    loadMentions: vi.fn(async () => []),
    saveMentions: vi.fn(async () => undefined),
    loadAuthoredPack: vi.fn(async (kind, id): Promise<AuthoredPack> => ({
      kind,
      id,
      name: "社区短语",
      version: "1.0.0",
      license: "CC-BY-4.0",
      author: "别人",
      description: null,
      phrases: [{ key: "hi", text: "你好" }],
    })),
    saveAuthoredPack,
    ...overrides,
  };
  return client;
}

function mount(
  client: PluginClient,
  {
    platform,
    preferences = defaultPluginPreferences,
    confirm = vi.fn(async () => true),
    quickPhraseMode = true,
  }: {
    platform?: string;
    preferences?: PluginPreferences;
    confirm?: (request: unknown) => Promise<boolean>;
    quickPhraseMode?: boolean;
  } = {},
) {
  const onError = vi.fn();
  render(
    <PluginsSection
      platform={platform}
      client={client}
      preferences={preferences}
      keySound
      music
      triggers
      quickPhraseMode={quickPhraseMode}
      commandMode
      wordbookPacks
      symbolSetPacks
      active
      onChange={vi.fn()}
      onError={onError}
      confirm={confirm}
    />,
  );
  return { onError, confirm };
}

async function startCreating(kind: string) {
  fireEvent.click(await screen.findByRole("button", { name: "新建插件" }));
  expect(screen.getByRole("heading", { name: "新建插件" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: kind }));
}

const type = (label: string, value: string) =>
  fireEvent.change(screen.getByLabelText(label), { target: { value } });

const saveButton = () => screen.getByRole("button", { name: /^(保存并安装|保存修改)$/ });

test("新建短语表：填好后保存并安装，打开它的详情并提醒还要启用和打开 K 模式", async () => {
  const client = editorClient();
  mount(client, { quickPhraseMode: false });
  await startCreating("短语表");
  expect(screen.getByRole("heading", { name: "新建短语表" })).toBeTruthy();
  expect((screen.getByLabelText("许可证") as HTMLInputElement).value).toBe("CC0-1.0");
  expect((screen.getByLabelText("版本") as HTMLInputElement).value).toBe("1.0.0");
  expect(saveButton()).toHaveProperty("disabled", true);

  type("插件名字", " 我的短语 ");
  type("第 1 条短语的编码", "dz");
  type("第 1 条短语的文字", "合成市合成区 1 号");
  fireEvent.click(screen.getByRole("button", { name: "添加一条" }));
  type("第 2 条短语的编码", "dh");
  type("第 2 条短语的文字", "010-0000");
  fireEvent.click(saveButton());

  await screen.findByRole("heading", { name: "我的短语" });
  expect(client.saveAuthoredPack).toHaveBeenCalledWith(
    {
      kind: "phrase_table",
      id: null,
      name: "我的短语",
      version: "1.0.0",
      license: "CC0-1.0",
      author: null,
      description: null,
      phrases: [
        { key: "dz", text: "合成市合成区 1 号" },
        { key: "dh", text: "010-0000" },
      ],
    },
    false,
  );
  expect(screen.getByRole("status").textContent).toContain("打开下面的「启用」");
  expect(screen.getByRole("status").textContent).toContain("K 模式");
  expect(screen.getByRole("switch", { name: /启用/ })).toBeTruthy();
  // 返回列表后提醒不再出现，新包在列表里。
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  const list = await screen.findByLabelText("已安装的插件");
  fireEvent.click(within(list).getByRole("button", { name: "我的短语" }));
  await screen.findByRole("heading", { name: "我的短语" });
  expect(screen.queryByText(/打开下面的「启用」/)).toBeNull();
});

test("新建指令表、符号集和单词本，各自交给宿主对应的内容", async () => {
  const client = editorClient();
  mount(client);
  await startCreating("指令表");
  type("插件名字", "签名");
  type("第 1 条指令的字母", "sig");
  type("第 1 条指令的标题", "签名");
  type("第 1 条指令要插入的文字", "{date} 张三");
  fireEvent.click(saveButton());
  await screen.findByRole("heading", { name: "签名" });
  expect(client.saveAuthoredPack).toHaveBeenLastCalledWith(
    expect.objectContaining({
      kind: "command_table",
      commands: [{ trigger: "sig", title: "签名", template: "{date} 张三" }],
    }),
    false,
  );
  expect(screen.getByRole("status").textContent).toContain("按 / 再输入指令字母");

  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  await startCreating("符号集");
  type("插件名字", "我的符号");
  fireEvent.change(screen.getByLabelText("第 1 组放在哪一页"), { target: { value: "kaomoji" } });
  type("第 1 组的标题", "开心");
  type("第 1 组的符号", "(^_^)\n\n  ( ﾟ∀ﾟ)  \n");
  fireEvent.click(screen.getByRole("button", { name: "添加一组" }));
  type("第 2 组的标题", "箭头");
  type("第 2 组的搜索关键词", "jiantou");
  type("第 2 组的符号", "→\n←");
  fireEvent.click(saveButton());
  await screen.findByRole("heading", { name: "我的符号" });
  expect(client.saveAuthoredPack).toHaveBeenLastCalledWith(
    expect.objectContaining({
      kind: "symbol_set",
      groups: [
        { tab: "kaomoji", title: "开心", keywords: null, items: ["(^_^)", "( ﾟ∀ﾟ)"] },
        { tab: "symbols", title: "箭头", keywords: "jiantou", items: ["→", "←"] },
      ],
    }),
    false,
  );
  expect(screen.getByRole("status").textContent).toContain("符号面板");

  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  await startCreating("单词本");
  type("插件名字", "我的单词");
  // 从表格复制过来的两列和三列都认，`#` 开头的行是注释。
  type("批量添加的内容", "# 合成词表\napple\t/ˈæp.əl/\tn. 苹果\nbanana\tn. 香蕉\n");
  fireEvent.click(screen.getByRole("button", { name: "添加到列表" }));
  expect((screen.getByLabelText("第 2 个单词") as HTMLInputElement).value).toBe("banana");
  expect(screen.queryByLabelText("第 3 个单词")).toBeNull();
  fireEvent.click(saveButton());
  await screen.findByRole("heading", { name: "我的单词" });
  expect(client.saveAuthoredPack).toHaveBeenLastCalledWith(
    expect.objectContaining({
      kind: "wordbook",
      words: [
        { word: "apple", phonetic: "/ˈæp.əl/", gloss: "n. 苹果" },
        { word: "banana", phonetic: null, gloss: "n. 香蕉" },
      ],
    }),
    false,
  );
});

test("表单按宿主的规则先指出第一处问题，改好之前不能保存", async () => {
  const client = editorClient();
  mount(client);
  await startCreating("短语表");
  type("插件名字", "合成");
  type("第 1 条短语的编码", "DH");
  type("第 1 条短语的文字", "电话");
  expect(screen.getByRole("alert").textContent).toBe(
    "第 1 条短语的编码只能是 1 到 32 个小写字母。",
  );
  expect(saveButton()).toHaveProperty("disabled", true);
  type("第 1 条短语的编码", "dh");
  type("许可证", "随便用");
  expect(screen.getByRole("alert").textContent).toContain("SPDX");
  type("许可证", "CC-BY-4.0");
  expect(screen.queryByRole("alert")).toBeNull();
  expect(saveButton()).toHaveProperty("disabled", false);

  // 批量添加读不出的行就地说明，不改动列表。
  type("批量添加的内容", "dh 电话\nnospace");
  fireEvent.click(screen.getByRole("button", { name: "添加到列表" }));
  expect(screen.getByRole("alert").textContent).toBe(
    "第 2 行要写成「编码 文字」，中间用空格隔开。",
  );
  expect(screen.queryByLabelText("第 2 条短语的编码")).toBeNull();
});

test("宿主拒绝时在表单里说明原因，草稿还在", async () => {
  const client = editorClient([], {
    saveAuthoredPack: vi.fn(async () => {
      throw { code: "plugin_exists", detail: null };
    }),
  });
  const { onError } = mount(client);
  await startCreating("短语表");
  type("插件名字", "合成");
  type("第 1 条短语的编码", "dh");
  type("第 1 条短语的文字", "电话");
  fireEvent.click(saveButton());
  expect((await screen.findByRole("alert")).textContent).toBe(
    "本机已经有一个同样 id 的插件，没有覆盖它。",
  );
  expect(onError).not.toHaveBeenCalled();
  expect(screen.getByRole("heading", { name: "新建短语表" })).toBeTruthy();
  expect((screen.getByLabelText("第 1 条短语的文字") as HTMLInputElement).value).toBe("电话");
});

test("宿主报告的规则原样显示，例如指令模板的格式写错了", async () => {
  const client = editorClient([], {
    saveAuthoredPack: vi.fn(async () => {
      throw { code: "plugin_invalid", detail: "指令 sig 的模板有不认识的占位符" };
    }),
  });
  mount(client);
  await startCreating("指令表");
  type("插件名字", "签名");
  type("第 1 条指令的字母", "sig");
  type("第 1 条指令的标题", "签名");
  type("第 1 条指令要插入的文字", "{date:%Q}");
  fireEvent.click(saveButton());
  expect((await screen.findByRole("alert")).textContent).toBe(
    "插件不符合要求（指令 sig 的模板有不认识的占位符）。",
  );
});

test("编辑社区装来的短语表：读出原内容，保存时替换同一个 id", async () => {
  const client = editorClient([userPhrases, officialPhrases]);
  mount(client);
  const list = await screen.findByLabelText("已安装的插件");
  // 内置的官方包不能编辑。
  fireEvent.click(await within(list).findByRole("button", { name: "LaTeX 符号" }));
  await screen.findByRole("heading", { name: "LaTeX 符号" });
  expect(screen.queryByRole("button", { name: /^编辑/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));

  fireEvent.click(
    await within(await screen.findByLabelText("已安装的插件")).findByRole("button", {
      name: "社区短语",
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑社区短语" }));
  expect(client.loadAuthoredPack).toHaveBeenCalledWith("phrase_table", "shared-phrases");
  await screen.findByRole("heading", { name: "编辑「社区短语」" });
  expect(((await screen.findByLabelText("第 1 条短语的文字")) as HTMLInputElement).value).toBe(
    "你好",
  );
  expect(saveButton()).toHaveProperty("disabled", true);
  type("第 1 条短语的文字", "你好呀");
  fireEvent.click(saveButton());
  await screen.findByRole("heading", { name: "社区短语" });
  expect(client.saveAuthoredPack).toHaveBeenCalledWith(
    {
      kind: "phrase_table",
      id: "shared-phrases",
      name: "社区短语",
      version: "1.0.0",
      license: "CC-BY-4.0",
      author: "别人",
      description: null,
      phrases: [{ key: "hi", text: "你好呀" }],
    },
    true,
  );
});

test("有未保存的修改时返回要先确认，取消就留在编辑器里", async () => {
  const confirm = vi.fn(async () => false);
  mount(editorClient([userPhrases]), { confirm });
  fireEvent.click(
    await within(await screen.findByLabelText("已安装的插件")).findByRole("button", {
      name: "社区短语",
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑社区短语" }));
  await screen.findByLabelText("第 1 条短语的文字");
  type("第 1 条短语的文字", "改了");
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  await waitFor(() => expect(confirm).toHaveBeenCalledOnce());
  expect(screen.getByRole("heading", { name: "编辑「社区短语」" })).toBeTruthy();

  confirm.mockResolvedValueOnce(true);
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  });
  // 编辑器从详情打开，放弃后回到详情。
  expect(await screen.findByRole("heading", { name: "社区短语" })).toBeTruthy();
});

test("读不出要编辑的包时说明原因", async () => {
  const client = editorClient([userPhrases], {
    loadAuthoredPack: vi.fn(async () => {
      throw { code: "plugin_not_editable", detail: "插件文件夹不存在" };
    }),
  });
  mount(client);
  fireEvent.click(
    await within(await screen.findByLabelText("已安装的插件")).findByRole("button", {
      name: "社区短语",
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑社区短语" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "这个插件不能在设置页编辑（插件文件夹不存在）。",
  );
});

test("宿主没有编辑器时不显示新建和编辑；macOS 插件库把新建放在导入旁边", async () => {
  const {
    loadAuthoredPack: _load,
    saveAuthoredPack: _save,
    ...plain
  } = editorClient([userPhrases]);
  mount(plain);
  fireEvent.click(
    await within(await screen.findByLabelText("已安装的插件")).findByRole("button", {
      name: "社区短语",
    }),
  );
  await screen.findByRole("heading", { name: "社区短语" });
  expect(screen.queryByRole("button", { name: /^编辑/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "返回我的插件" }));
  expect(screen.queryByRole("button", { name: "新建插件" })).toBeNull();
  cleanup();

  mount(editorClient(), { platform: "macos" });
  fireEvent.click(await screen.findByRole("button", { name: "新建插件" }));
  expect(screen.getByRole("button", { name: "单词本" })).toBeTruthy();
});

test("草稿整理和规则：空行不保存，单词本的名字和单词按词书的上限", () => {
  const draft = {
    ...emptyDraft("wordbook"),
    name: "x".repeat(65),
    words: [
      { word: "apple", phonetic: " ", gloss: "苹果" },
      { word: "", phonetic: "", gloss: "" },
    ],
  };
  expect(draftIssue(draft)).toBe("单词本的名字不能超过 64 个字。");
  const named = { ...draft, name: "合成" };
  expect(draftIssue(named)).toBeNull();
  expect(packFromDraft(named).words).toEqual([{ word: "apple", phonetic: null, gloss: "苹果" }]);
  expect(draftIssue({ ...named, words: [{ word: "#tag", phonetic: "", gloss: "x" }] })).toBe(
    "第 1 个单词不能以 # 开头。",
  );
  expect(
    draftIssue({
      ...named,
      words: [
        { word: "a", phonetic: "", gloss: "x" },
        { word: "a", phonetic: "", gloss: "y" },
      ],
    }),
  ).toBe("「a」出现了不止一次。");
  expect(draftIssue({ ...emptyDraft("phrase_table"), name: "合成" })).toBe("至少要有一条短语。");
  expect(
    draftIssue({
      ...emptyDraft("command_table"),
      name: "合成",
      commands: [{ trigger: "x", title: "x", template: "{clipboard}" }],
    }),
  ).toBe("第 1 条指令的花括号里只能写 {date}、{time} 或 {weekday}。");
  expect(
    draftIssue({
      ...emptyDraft("symbol_set"),
      name: "合成",
      groups: [
        { tab: "symbols", title: "同名", keywords: "", items: "→" },
        { tab: "symbols", title: "同名", keywords: "", items: "←" },
      ],
    }),
  ).toBe("第 2 组的标题和同一页的另一组重复了。");
  expect(parsePhraseLines("dz  合成 地址 \n\n dh 电话").rows).toEqual([
    { key: "dz", text: "合成 地址" },
    { key: "dh", text: "电话" },
  ]);
  expect(parseWordLines("a\tb\tc\td").issue).toContain("第 1 行");
});
