// keyFromEvent 和 createShiftTap 的单元测试，直接对源码运行：node --test packages/web-engine/test/keys.test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";
import { KeyKind, createShiftTap, keyFromEvent, osImeIntercepting, packKey } from "../src/keys.js";

const ev = (key, extra = {}) => ({
  key,
  code: "",
  repeat: false,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false,
  isComposing: false,
  keyCode: 0,
  getModifierState: () => false,
  ...extra,
});

test("letters, shifted letters, digits and punctuation carry their ASCII byte", () => {
  assert.equal(keyFromEvent(ev("n")), packKey(KeyKind.Letter, 0x6e));
  assert.equal(keyFromEvent(ev("N", { shiftKey: true })), packKey(KeyKind.ShiftLetter, 0x4e));
  assert.equal(keyFromEvent(ev("7")), packKey(KeyKind.Digit, 0x37));
  assert.equal(keyFromEvent(ev(",")), packKey(KeyKind.Punct, 0x2c));
  assert.equal(keyFromEvent(ev("'")), packKey(KeyKind.Punct, 0x27));
});

test("- and = are paging keys that carry the punctuation they borrow", () => {
  assert.equal(keyFromEvent(ev("-")), packKey(KeyKind.PagePrev, 0x2d));
  assert.equal(keyFromEvent(ev("=")), packKey(KeyKind.PageNext, 0x3d));
});

test("named keys map whatever the engine is doing", () => {
  assert.equal(keyFromEvent(ev(" ")), packKey(KeyKind.Space));
  assert.equal(keyFromEvent(ev("Enter")), packKey(KeyKind.Enter));
  assert.equal(keyFromEvent(ev("Escape")), packKey(KeyKind.Escape));
  assert.equal(keyFromEvent(ev("PageUp")), packKey(KeyKind.PagePrev));
  assert.equal(keyFromEvent(ev("ArrowDown")), packKey(KeyKind.PageNext));
});

test("left and right arrows move the highlight only while composing", () => {
  assert.equal(keyFromEvent(ev("ArrowLeft")), null);
  assert.equal(keyFromEvent(ev("ArrowLeft"), { composing: true }), packKey(KeyKind.HighlightPrev));
  assert.equal(keyFromEvent(ev("ArrowRight"), { composing: true }), packKey(KeyKind.HighlightNext));
});

test("backspace deletes a word with Ctrl or Alt and is left alone with Cmd", () => {
  assert.equal(keyFromEvent(ev("Backspace")), packKey(KeyKind.Backspace));
  assert.equal(keyFromEvent(ev("Backspace", { ctrlKey: true })), packKey(KeyKind.BackspaceWord));
  assert.equal(keyFromEvent(ev("Backspace", { altKey: true })), packKey(KeyKind.BackspaceWord));
  assert.equal(keyFromEvent(ev("Backspace", { metaKey: true })), null);
});

test("shortcuts, Tab, non-ASCII and repeats of typing keys are not the engine's", () => {
  assert.equal(keyFromEvent(ev("c", { ctrlKey: true })), null);
  assert.equal(keyFromEvent(ev("v", { metaKey: true })), null);
  assert.equal(keyFromEvent(ev("a", { altKey: true })), null);
  assert.equal(keyFromEvent(ev("Tab")), null);
  assert.equal(keyFromEvent(ev("F5")), null);
  assert.equal(keyFromEvent(ev("é")), null);
  assert.equal(keyFromEvent(ev("a", { repeat: true })), null);
  assert.equal(keyFromEvent(ev("Backspace", { repeat: true })), packKey(KeyKind.Backspace));
});

test("a symbol typed with Alt or AltGr reaches the engine as that symbol", () => {
  assert.equal(keyFromEvent(ev("@", { altKey: true })), packKey(KeyKind.Punct, 0x40));
  assert.equal(keyFromEvent(ev("@", { ctrlKey: true, altKey: true, getModifierState: (m) => m === "AltGraph" })), packKey(KeyKind.Punct, 0x40));
});

test("typed overrides e.key for a page that maps the layout itself", () => {
  assert.equal(keyFromEvent(ev("j"), { typed: "h" }), packKey(KeyKind.Letter, 0x68));
  assert.equal(keyFromEvent(ev("j"), { typed: null }), packKey(KeyKind.Letter, 0x6a));
});

test("osImeIntercepting recognises an OS input method converting the key", () => {
  assert.equal(osImeIntercepting(ev("Process")), true);
  assert.equal(osImeIntercepting(ev("a", { keyCode: 229 })), true);
  assert.equal(osImeIntercepting(ev("a", { isComposing: true })), true);
  assert.equal(osImeIntercepting(ev("a")), false);
});

test("Shift tap fires only for a Shift released with no other key in between", () => {
  const tap = createShiftTap();
  tap.down(ev("Shift"));
  assert.equal(tap.up(ev("Shift")), true);
  tap.down(ev("Shift"));
  tap.down(ev("A"));
  assert.equal(tap.up(ev("Shift")), false);
  assert.equal(tap.up(ev("Shift")), false);
});
