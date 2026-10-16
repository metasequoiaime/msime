// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  CommunityHomePage,
  type CommunityResourceClient,
  type CommunitySkinClient,
} from "@msime/ui";

afterEach(cleanup);

function resources(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [], has_more: false }),
    detail: vi.fn(),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

function skins(): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    setCategory: vi.fn(),
    finishTrial: vi.fn(),
  };
}

test("home keeps its selected category until the destination remounts it", async () => {
  const client = resources();
  const view = render(
    <CommunityHomePage key="first" resources={client} theme="light" initialCategory="reply" />,
  );
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("reply", "", "", 0));
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("dictionary", "", "", 0));

  view.rerender(
    <CommunityHomePage key="first" resources={client} theme="dark" initialCategory="reply" />,
  );
  expect(screen.getByRole("tab", { name: "词库" }).getAttribute("aria-selected")).toBe("true");

  view.rerender(
    <CommunityHomePage key="second" resources={client} theme="dark" initialCategory="reply" />,
  );
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("reply", "", "", 0));
  expect(screen.getByRole("tab", { name: "回复模板" }).getAttribute("aria-selected")).toBe("true");
});

test("switching resource categories preserves the gallery search and chosen scope", async () => {
  const client = resources();
  render(<CommunityHomePage resources={client} theme="light" initialCategory="dictionary" />);
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith("dictionary", "", "", 0));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索词库" }), {
    target: { value: " 合成 查询 " },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() =>
    expect(client.list).toHaveBeenLastCalledWith("dictionary", "", " 合成 查询 ", 0),
  );
  fireEvent.click(screen.getByRole("button", { name: "我的作品" }));
  await waitFor(() =>
    expect(client.list).toHaveBeenLastCalledWith("dictionary", "mine", " 合成 查询 ", 0),
  );
  fireEvent.click(screen.getByRole("tab", { name: "回复模板" }));
  await waitFor(() =>
    expect(client.list).toHaveBeenLastCalledWith("reply", "mine", " 合成 查询 ", 0),
  );
  expect((screen.getByRole("textbox", { name: "搜索回复模板" }) as HTMLInputElement).value).toBe(
    " 合成 查询 ",
  );
});

test("returning to the skin gallery remounts it with the original mine filter", async () => {
  const skinClient = skins();
  const resourceClient = resources();
  render(
    <CommunityHomePage
      skins={skinClient}
      resources={resourceClient}
      theme="dark"
      initialMine
      initialScope="saved"
    />,
  );
  await waitFor(() => expect(skinClient.list).toHaveBeenLastCalledWith(0, "", true, null));
  expect(resourceClient.list).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  await waitFor(() =>
    expect(resourceClient.list).toHaveBeenLastCalledWith("dictionary", "saved", "", 0),
  );
  fireEvent.click(screen.getByRole("tab", { name: "皮肤" }));
  await waitFor(() => expect(skinClient.list).toHaveBeenCalledTimes(2));
  expect(skinClient.list).toHaveBeenLastCalledWith(0, "", true, null);
});
