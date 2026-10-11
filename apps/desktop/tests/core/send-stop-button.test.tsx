// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { SendStopButton } from "../../../../packages/ui/src/core/send-stop-button";

test("sends or stops while keeping the composer focused", () => {
  const onSend = vi.fn();
  const onStop = vi.fn();
  const { rerender } = render(
    <SendStopButton
      className="composer-action"
      sending={false}
      sendDisabled
      onSend={onSend}
      onStop={onStop}
    />,
  );
  const send = screen.getByRole<HTMLButtonElement>("button", { name: "发送" });
  expect(send.disabled).toBe(true);
  expect(send.className).toBe("composer-action");
  rerender(<SendStopButton sending={false} onSend={onSend} onStop={onStop} />);
  const enabledSend = screen.getByRole<HTMLButtonElement>("button", { name: "发送" });
  const pointerDown = new Event("pointerdown", { bubbles: true, cancelable: true });
  enabledSend.dispatchEvent(pointerDown);
  expect(pointerDown.defaultPrevented).toBe(true);
  const mouseDown = new MouseEvent("mousedown", { bubbles: true, cancelable: true });
  enabledSend.dispatchEvent(mouseDown);
  expect(mouseDown.defaultPrevented).toBe(true);
  fireEvent.click(enabledSend);
  expect(onSend).toHaveBeenCalledTimes(1);
  rerender(<SendStopButton sending sendDisabled onSend={onSend} onStop={onStop} />);
  const stop = screen.getByRole<HTMLButtonElement>("button", { name: "停止" });
  expect(stop.disabled).toBe(false);
  fireEvent.click(stop);
  expect(onStop).toHaveBeenCalledTimes(1);
  expect(onSend).toHaveBeenCalledTimes(1);
});
