// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { HostActionButton } from "../../../../packages/ui/src/keyboard/HostActionButton";

afterEach(cleanup);

test("missing host capability disables the action", () => {
  render(<HostActionButton label="复制群号" />);
  expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(true);
});

test("host action remains available after StrictMode effect replay", async () => {
  const action = vi.fn().mockResolvedValue(undefined);
  render(
    <StrictMode>
      <HostActionButton label="打开" action={action} />
    </StrictMode>,
  );

  fireEvent.click(screen.getByRole("button"));
  await waitFor(() => expect(action).toHaveBeenCalledOnce());
});

test("copy waits for completion and suppresses duplicate requests", async () => {
  let finish!: () => void;
  const action = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  render(<HostActionButton label="复制群号" success="已复制" action={action} />);
  const button = screen.getByRole("button");
  fireEvent.click(button);
  fireEvent.click(button);
  expect(action).toHaveBeenCalledTimes(1);
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryByRole("status")).toBeNull();
  finish();
  expect((await screen.findByRole("status")).textContent).toBe("已复制");
  await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
});

test("host rejection is redacted and a subsequent attempt can succeed", async () => {
  const action = vi
    .fn()
    .mockRejectedValueOnce(new Error("synthetic-host-detail"))
    .mockResolvedValue(undefined);
  render(<HostActionButton label="打开" success="完成" action={action} />);
  fireEvent.click(screen.getByRole("button"));
  expect((await screen.findByRole("alert")).textContent).toBe("操作失败，请重试。");
  expect(screen.queryByText(/synthetic-host-detail/)).toBeNull();
  fireEvent.click(screen.getByRole("button"));
  await screen.findByRole("status");
  expect(screen.queryByRole("alert")).toBeNull();
  expect(action).toHaveBeenCalledTimes(2);
});

test("synchronous host failure is handled too", async () => {
  render(
    <HostActionButton
      label="打开"
      action={() => {
        throw new Error("synthetic-host-detail");
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button"));
  await screen.findByRole("alert");
});
