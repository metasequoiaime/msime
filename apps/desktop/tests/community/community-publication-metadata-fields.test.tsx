// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunityPublicationMetadataFieldsProps = {
  name: string;
  description: string;
  agreed: boolean;
  busy?: boolean;
  nameLabel: string;
  nameAriaLabel: string;
  descriptionLabel: string;
  descriptionAriaLabel: string;
  descriptionRows?: number;
  agreementText: string;
  agreementAriaLabel?: string;
  agreementClassName?: string;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onAgreedChange: (agreed: boolean) => void;
};

test("shared publication metadata fields preserve labels, edits, and busy state", () => {
  const Fields = (
    ui as unknown as {
      CommunityPublicationMetadataFields: ComponentType<CommunityPublicationMetadataFieldsProps>;
    }
  ).CommunityPublicationMetadataFields;
  expect(Fields).toBeDefined();

  const onNameChange = vi.fn();
  const onDescriptionChange = vi.fn();
  const onAgreedChange = vi.fn();
  const { rerender } = render(
    <Fields
      name="初始名称"
      description="初始说明"
      agreed={false}
      nameLabel="作品名称"
      nameAriaLabel="社区作品名称"
      descriptionLabel="作品说明"
      descriptionAriaLabel="社区作品说明"
      descriptionRows={3}
      agreementText="合成授权文字"
      onNameChange={onNameChange}
      onDescriptionChange={onDescriptionChange}
      onAgreedChange={onAgreedChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("社区作品名称"), {
    target: { value: "更新名称" },
  });
  fireEvent.change(screen.getByLabelText("社区作品说明"), {
    target: { value: "更新说明" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));

  expect(onNameChange).toHaveBeenCalledWith("更新名称");
  expect(onDescriptionChange).toHaveBeenCalledWith("更新说明");
  expect(onAgreedChange).toHaveBeenCalledWith(true);
  expect(screen.getByLabelText("社区作品说明")).toHaveProperty("rows", 3);
  expect(screen.getByText("合成授权文字")).toBeTruthy();

  rerender(
    <Fields
      name="初始名称"
      description="初始说明"
      agreed
      busy
      nameLabel="名称"
      nameAriaLabel="发布插件名称"
      descriptionLabel="说明"
      descriptionAriaLabel="发布插件说明"
      agreementText="合成插件授权文字"
      agreementAriaLabel="确认插件发布权利"
      onNameChange={onNameChange}
      onDescriptionChange={onDescriptionChange}
      onAgreedChange={onAgreedChange}
    />,
  );

  expect(screen.getByLabelText("发布插件名称")).toHaveProperty("disabled", true);
  expect(screen.getByLabelText("发布插件说明")).toHaveProperty("disabled", true);
  expect(screen.getByRole("checkbox", { name: "确认插件发布权利" })).toHaveProperty(
    "disabled",
    true,
  );
});
