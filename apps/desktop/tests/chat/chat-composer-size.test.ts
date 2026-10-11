// @vitest-environment jsdom
import { expect, test } from "vitest";
import { resizeChatComposer } from "../../../../packages/ui/src/chat/chat-composer-size";

test("caps composer growth at four lines and shrinks after content is cleared", () => {
  const field = document.createElement("textarea");
  Object.defineProperty(field, "scrollHeight", { configurable: true, value: 180 });
  resizeChatComposer(field);
  expect(field.style.height).toBe("104px");

  Object.defineProperty(field, "scrollHeight", {
    configurable: true,
    get: () => (field.style.height === "auto" ? 22 : 104),
  });
  resizeChatComposer(field);
  expect(field.style.height).toBe("22px");
  expect(() => resizeChatComposer(null)).not.toThrow();
});
