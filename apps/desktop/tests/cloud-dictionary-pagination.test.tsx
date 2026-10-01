// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type PaginationProps = {
  offset: number;
  hasMore: boolean;
  busy?: boolean;
  onPrevious: () => void;
  onNext: () => void;
};

test("cloud dictionary pagination shares page state and navigation actions", () => {
  const Pagination = (
    ui as unknown as { CloudDictionaryPagination: ComponentType<PaginationProps> }
  ).CloudDictionaryPagination;
  expect(Pagination).toBeDefined();

  const onPrevious = vi.fn();
  const onNext = vi.fn();
  render(
    <Pagination offset={100} hasMore onPrevious={onPrevious} onNext={onNext} />,
  );

  expect(screen.getByText("第 2 页")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "上一页" }));
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect(onPrevious).toHaveBeenCalledOnce();
  expect(onNext).toHaveBeenCalledOnce();
});

test("cloud dictionary pagination disables unavailable directions", () => {
  const Pagination = (
    ui as unknown as { CloudDictionaryPagination: ComponentType<PaginationProps> }
  ).CloudDictionaryPagination;
  render(<Pagination offset={0} hasMore={false} onPrevious={vi.fn()} onNext={vi.fn()} />);

  expect(screen.getByRole("button", { name: "上一页" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "下一页" })).toHaveProperty("disabled", true);
});
