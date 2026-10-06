import { afterEach, expect, test, vi } from "vitest";
import { installConditionalFonts } from "../../../../packages/ui/src/skin/conditional-fonts";

afterEach(() => vi.unstubAllGlobals());
function query(matches: boolean) {
  const listeners = new Set<() => void>();
  const value = {
    matches,
    addEventListener: vi.fn((_: string, fn: () => void) => listeners.add(fn)),
    removeEventListener: vi.fn((_: string, fn: () => void) => listeners.delete(fn)),
  };
  return {
    media: value as unknown as MediaQueryList,
    listeners,
    // The copy is the point: installConditionalFonts removes listeners from
    // this Set while the change is being dispatched, and iterating the live Set
    // would then skip the ones a previous callback removed.
    change(next: boolean) {
      value.matches = next;
      // oxlint-disable-next-line unicorn/no-useless-spread
      for (const fn of [...listeners]) fn();
    },
  };
}
test("nested conditions preserve source order, share listeners and clean up only owned fonts", () => {
  const outside = {} as FontFace,
    first = {} as FontFace,
    second = {} as FontFace;
  const fonts = new Set([outside]);
  vi.stubGlobal("document", { fonts });
  const wide = query(false),
    supportedSize = query(true);
  const remove = installConditionalFonts([
    { face: first, media: [wide.media, supportedSize.media] },
    { face: second, media: [supportedSize.media] },
  ]);
  expect([...fonts]).toEqual([outside, second]);
  expect(supportedSize.media.addEventListener).toHaveBeenCalledTimes(1);
  wide.change(true);
  expect([...fonts]).toEqual([outside, first, second]);
  supportedSize.change(false);
  expect([...fonts]).toEqual([outside]);
  supportedSize.change(true);
  expect([...fonts]).toEqual([outside, first, second]);
  remove();
  remove();
  wide.change(false);
  wide.change(true);
  expect([...fonts]).toEqual([outside]);
  expect(wide.listeners.size + supportedSize.listeners.size).toBe(0);
});
test("registration failures roll back fonts and listeners", () => {
  const outside = {} as FontFace,
    first = {} as FontFace,
    bad = {} as FontFace;
  const fonts = new Set([outside]);
  vi.spyOn(fonts, "add").mockImplementation((face) => {
    if (face === bad) throw Error("synthetic failure");
    return Set.prototype.add.call(fonts, face);
  });
  vi.stubGlobal("document", { fonts });
  const media = query(true);
  expect(() =>
    installConditionalFonts([
      { face: first, media: [media.media] },
      { face: bad, media: [] },
    ]),
  ).toThrow("font installation failed");
  expect([...fonts]).toEqual([outside]);
  expect(media.listeners.size).toBe(0);
});
