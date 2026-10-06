// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import type { KeyboardEvent } from "react";
import { handleCommunityPublishKeyDown } from "@msime/ui";

function eventFor(target: HTMLInputElement, key = "Enter") {
  return {
    key,
    target,
    preventDefault: vi.fn(),
  } as unknown as KeyboardEvent<HTMLElement>;
}

test("community publish key handler submits text inputs and prevents browser submit", () => {
  const submit = vi.fn();
  const event = eventFor(document.createElement("input"));

  handleCommunityPublishKeyDown(event, submit);

  expect(event.preventDefault).toHaveBeenCalledOnce();
  expect(submit).toHaveBeenCalledOnce();
});

test("community publish key handler ignores checkboxes and other keys", () => {
  const submit = vi.fn();
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  const checkboxEvent = eventFor(checkbox);
  const escapeEvent = eventFor(document.createElement("input"), "Escape");

  handleCommunityPublishKeyDown(checkboxEvent, submit);
  handleCommunityPublishKeyDown(escapeEvent, submit);

  expect(checkboxEvent.preventDefault).toHaveBeenCalledOnce();
  expect(escapeEvent.preventDefault).not.toHaveBeenCalled();
  expect(submit).not.toHaveBeenCalled();
});
