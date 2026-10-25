import { expect, test } from "vitest";
import { resolveKoreanLayout } from "../../../../packages/ui/src/settings/korean-preferences";

test("韩语缺省和未知布局回退到两套式", () => {
  expect(resolveKoreanLayout()).toBe("dubeolsik");
  expect(resolveKoreanLayout({})).toBe("dubeolsik");
  expect(resolveKoreanLayout(JSON.parse('{"layout":"future"}'))).toBe("dubeolsik");
  expect(resolveKoreanLayout({ layout: "romaja" })).toBe("romaja");
});
