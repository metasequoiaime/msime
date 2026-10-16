// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  CommunityPluginPublishDialog,
  type CommunityPluginPublishDialogProps,
  type PluginCatalogResult,
} from "@msime/ui";
import { CommunityPluginPublishDialog as LegacyDialog } from "../../../../packages/ui/src/community/community-plugins";
import { CommunityPluginPublishDialog as DirectDialog } from "../../../../packages/ui/src/community/community-plugin-publish-dialog";

afterEach(cleanup);

test("包出口与旧图库出口指向独立对话框", () => {
  expect(CommunityPluginPublishDialog).toBe(DirectDialog);
  expect(LegacyDialog).toBe(DirectDialog);
});

test("独立入口只要求预检和发布能力，空目录可以关闭", async () => {
  const props: CommunityPluginPublishDialogProps = {
    client: { packPreview: vi.fn(), publish: vi.fn() },
    localPlugins: vi.fn(async (): Promise<PluginCatalogResult> => ({ packages: [], issues: [] })),
    onClose: vi.fn(),
    onPublished: vi.fn(),
  };
  render(<CommunityPluginPublishDialog {...props} />);
  await screen.findByText(/还没有可发布的插件/);
  expect(screen.queryByRole("combobox")).toBeNull();
  expect((screen.getByRole("button", { name: "公开发布" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(props.onClose).toHaveBeenCalledOnce();
  expect(props.client.packPreview).not.toHaveBeenCalled();
  expect(props.client.publish).not.toHaveBeenCalled();
});

test("独立入口保留发布登录操作", async () => {
  const props: CommunityPluginPublishDialogProps = {
    client: {
      packPreview: vi.fn(async () => ({
        suggestedName: "合成雨声",
        suggestedDescription: "合成说明",
        version: "1.0.0",
        license: "MIT",
        fileCount: 2,
        size: 2048,
      })),
      publish: vi.fn().mockRejectedValue({ code: "community_unauthorized" }),
    },
    localPlugins: vi.fn(async (): Promise<PluginCatalogResult> => ({
      packages: [
        {
          kind: "sound",
          id: "synthetic-rain",
          name: "合成雨声",
          version: "1.0.0",
          license: "MIT",
          author: null,
          description: null,
          builtin: false,
        },
      ],
      issues: [],
    })),
    onClose: vi.fn(),
    onPublished: vi.fn(),
    onLogin: vi.fn(),
  };
  render(<CommunityPluginPublishDialog {...props} />);
  await screen.findByRole("textbox", { name: "发布插件名称" });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  fireEvent.click(await screen.findByRole("button", { name: "去登录" }));
  await waitFor(() => expect(props.onLogin).toHaveBeenCalledOnce());
  expect(props.onPublished).not.toHaveBeenCalled();
});
