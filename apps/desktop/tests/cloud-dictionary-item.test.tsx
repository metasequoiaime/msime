// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type ItemProps = {
  ariaLabel: string;
  busy?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
  actions?: ReactNode;
};

test("shared cloud dictionary item keeps the main action and action group in one item shell", () => {
  const Item = (ui as unknown as { CloudDictionaryItem: ComponentType<ItemProps> })
    .CloudDictionaryItem;
  expect(Item).toBeDefined();

  const onClick = vi.fn();
  const onAction = vi.fn();
  render(
    <Item
      ariaLabel="调频候选 你好"
      onClick={onClick}
      actions={<button onClick={onAction}>调频</button>}
    >
      <strong>你好</strong>
      <small>ni · 权重 100</small>
    </Item>,
  );

  expect(screen.getByRole("article")).toBeTruthy();
  expect(screen.getByRole("button", { name: "调频候选 你好" })).toBeTruthy();
  expect(screen.getByText("ni · 权重 100")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "调频候选 你好" }));
  fireEvent.click(screen.getByRole("button", { name: "调频" }));
  expect(onClick).toHaveBeenCalledOnce();
  expect(onAction).toHaveBeenCalledOnce();
});

test("shared cloud dictionary item disables its main action when busy or explicitly disabled", () => {
  const Item = (ui as unknown as { CloudDictionaryItem: ComponentType<ItemProps> })
    .CloudDictionaryItem;
  const { rerender } = render(
    <Item ariaLabel="候选" onClick={vi.fn()} busy>
      候选
    </Item>,
  );
  expect(screen.getByRole("button", { name: "候选" })).toHaveProperty("disabled", true);

  rerender(
    <Item ariaLabel="候选" onClick={vi.fn()} disabled>
      候选
    </Item>,
  );
  expect(screen.getByRole("button", { name: "候选" })).toHaveProperty("disabled", true);
});
