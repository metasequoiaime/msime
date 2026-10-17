// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunityResourceEditor, type CommunityResourceEditorProps } from "@msime/ui";

afterEach(cleanup);

test("公共资源编辑器用最小客户端独立发布与关闭", async () => {
  const props: CommunityResourceEditorProps = {
    client: { publish: vi.fn(async () => {}) },
    kind: "reply",
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunityResourceEditor {...props} />);
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "合成回复" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区回复提示词" }), {
    target: { value: "合成提示" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(props.onPublished).toHaveBeenCalledOnce());
  expect(props.client.publish).toHaveBeenCalledWith(
    expect.any(String),
    "reply",
    "合成回复",
    "",
    { prompt: "合成提示" },
    0,
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(props.onClose).toHaveBeenCalledOnce();
});

function fillEntry(code: string, word: string) {
  fireEvent.change(screen.getByRole("textbox", { name: "社区词条编码" }), {
    target: { value: code },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区词条文字" }), {
    target: { value: word },
  });
  fireEvent.click(screen.getByRole("button", { name: "添加词条" }));
}

test("发布词库时拦下来自搜狗细胞词库的词条，并说明原因", async () => {
  // 合成数据：「安痛定」当作细胞词库里的词，「水杉」是用户自己整理的。
  const thirdPartyMatches = vi.fn(async (entries: { word: string }[]) =>
    entries.flatMap((entry, index) => (entry.word === "安痛定" ? [index] : [])),
  );
  const props: CommunityResourceEditorProps = {
    client: { publish: vi.fn(async () => {}) },
    kind: "dictionary",
    localDictionary: { thirdPartyMatches },
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunityResourceEditor {...props} />);
  expect(screen.getByText(/只发布你自己整理的词条/)).toBeTruthy();
  fillEntry("an'tong'ding", "安痛定");
  expect(await screen.findByText(/“安痛定”来自你导入的搜狗细胞词库，不能发布到社区/)).toBeTruthy();
  expect(screen.getByLabelText("待发布词条 0/128")).toBeTruthy();
  fillEntry("shui'shan", "水杉");
  await waitFor(() => expect(screen.getByLabelText("待发布词条 1/128")).toBeTruthy());
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "合成词库" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(props.onPublished).toHaveBeenCalledOnce());
  expect(thirdPartyMatches).toHaveBeenLastCalledWith([
    { kind: "pinyin", code: "shui'shan", word: "水杉", weight: 100000 },
  ]);
});

test("更新旧作品时，整份词条在发布前再核对一次", async () => {
  const thirdPartyMatches = vi.fn(async () => [1]);
  const props: CommunityResourceEditorProps = {
    client: { publish: vi.fn(async () => {}) },
    kind: "dictionary",
    existing: {
      id: "00000000-0000-4000-8000-000000000001",
      kind: "dictionary",
      name: "旧作品",
      description: "",
      content: {
        entries: [
          { kind: "pinyin", code: "shui'shan", word: "水杉", weight: 1 },
          { kind: "pinyin", code: "an'tong'ding", word: "安痛定", weight: 1 },
        ],
      },
      revision: 2,
      saves: 0,
      saved: false,
      owned: true,
      rating_count: 0,
      rating_average: 0,
      my_rating: 0,
    } as CommunityResourceEditorProps["existing"],
    localDictionary: { thirdPartyMatches },
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunityResourceEditor {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "发布新版本" }));
  expect(await screen.findByText(/“安痛定”来自你导入的搜狗细胞词库/)).toBeTruthy();
  expect(props.client.publish).not.toHaveBeenCalled();
});

test("宿主核对失败或没有核对能力时照常发布", async () => {
  const props: CommunityResourceEditorProps = {
    client: { publish: vi.fn(async () => {}) },
    kind: "dictionary",
    localDictionary: { thirdPartyMatches: vi.fn(async () => Promise.reject(new Error("io"))) },
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunityResourceEditor {...props} />);
  fillEntry("shui'shan", "水杉");
  await waitFor(() => expect(screen.getByLabelText("待发布词条 1/128")).toBeTruthy());
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "合成词库" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(props.onPublished).toHaveBeenCalledOnce());
});
