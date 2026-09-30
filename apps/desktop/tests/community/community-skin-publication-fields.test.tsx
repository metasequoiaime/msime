// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunitySkinPublicationFieldsProps = {
  name: string;
  description: string;
  agreed: boolean;
  busy?: boolean;
  agreementText: string;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onAgreedChange: (agreed: boolean) => void;
};

test("shared skin publication fields forward edits and agreement state", () => {
  const Fields = (
    ui as unknown as {
      CommunitySkinPublicationFields: ComponentType<CommunitySkinPublicationFieldsProps>;
    }
  ).CommunitySkinPublicationFields;
  expect(Fields).toBeDefined();

  const onNameChange = vi.fn();
  const onDescriptionChange = vi.fn();
  const onAgreedChange = vi.fn();
  render(
    <Fields
      name="初始名称"
      description="初始说明"
      agreed={false}
      agreementText="合成授权文字"
      onNameChange={onNameChange}
      onDescriptionChange={onDescriptionChange}
      onAgreedChange={onAgreedChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("发布皮肤名称"), {
    target: { value: "更新名称" },
  });
  fireEvent.change(screen.getByLabelText("发布设计说明"), {
    target: { value: "更新说明" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));

  expect(onNameChange).toHaveBeenCalledWith("更新名称");
  expect(onDescriptionChange).toHaveBeenCalledWith("更新说明");
  expect(onAgreedChange).toHaveBeenCalledWith(true);
  expect(screen.getByText("合成授权文字")).toBeTruthy();
});
