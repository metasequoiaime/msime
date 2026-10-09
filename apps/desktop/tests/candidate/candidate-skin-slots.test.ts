import { expect, test } from "vitest";
import {
  applyCandidateSkin,
  candidateSkinFor,
  customDrawnBase,
  removeCandidateSkin,
  skinDrawsIn,
  skinSlot,
  type ThemeAppearance,
} from "../../../../packages/ui/src/theme/global-theme";

// 合成的皮肤：sakura、paper-notes 是浅色底，dusk、starry 是深色底，mist 跟随系统。
const slots: Record<string, ThemeAppearance | null> = {
  sakura: "light",
  "paper-notes": "light",
  dusk: "dark",
  starry: "dark",
  mist: null,
};
const slotOf = (id: string) => slots[id];

test("a skin belongs to the slot of its base", () => {
  expect(skinSlot("paper")).toBe("light");
  expect(skinSlot("light")).toBe("light");
  expect(skinSlot("night")).toBe("dark");
  expect(skinSlot("ink")).toBe("dark");
  expect(skinSlot("shuishan")).toBe("dark");
  expect(skinSlot("system")).toBeNull();
  expect(skinDrawsIn("paper", false)).toBe(true);
  expect(skinDrawsIn("paper", true)).toBe(false);
  expect(skinDrawsIn("night", false)).toBe(false);
  expect(skinDrawsIn("system", true)).toBe(true);
});

test("dark mode takes the dark slot and falls back to the light one", () => {
  expect(candidateSkinFor({ candidate_skin: "sakura", candidate_skin_dark: "dusk" }, true)).toBe(
    "dusk",
  );
  expect(candidateSkinFor({ candidate_skin: "sakura", candidate_skin_dark: "dusk" }, false)).toBe(
    "sakura",
  );
  expect(candidateSkinFor({ candidate_skin: "starry" }, true)).toBe("starry");
  expect(candidateSkinFor({ candidate_skin_dark: "dusk" }, false)).toBeNull();
  expect(candidateSkinFor(undefined, true)).toBeNull();
});

test("applying a skin fills only its own slot", () => {
  const light = applyCandidateSkin({ candidate_skin_dark: "dusk" }, "sakura", "paper", slotOf);
  expect(light).toMatchObject({
    base: "paper",
    candidate_skin: "sakura",
    candidate_skin_dark: "dusk",
  });
  const dark = applyCandidateSkin(light, "starry", "night", slotOf);
  expect(dark).toMatchObject({
    base: "night",
    candidate_skin: "sakura",
    candidate_skin_dark: "starry",
  });
  const both = applyCandidateSkin(dark, "mist", "system", slotOf);
  expect(both).toMatchObject({
    base: "system",
    candidate_skin: "mist",
    candidate_skin_dark: "mist",
  });
});

test("a legacy dark skin is kept when a light skin is applied", () => {
  // 旧文档只有一款深色皮肤，存在 candidate_skin 里。
  const next = applyCandidateSkin(
    { base: "night", candidate_skin: "starry" },
    "sakura",
    "paper",
    slotOf,
  );
  expect(next).toMatchObject({ candidate_skin: "sakura", candidate_skin_dark: "starry" });
  // 原来是浅色皮肤时不挪：深色槽位保持空着。
  const replaced = applyCandidateSkin({ candidate_skin: "paper-notes" }, "sakura", "paper", slotOf);
  expect(replaced.candidate_skin).toBe("sakura");
  expect(replaced.candidate_skin_dark).toBeUndefined();
});

test("a new dark skin replaces a legacy dark skin in candidate_skin", () => {
  const next = applyCandidateSkin(
    { base: "night", candidate_skin: "starry" },
    "dusk",
    "night",
    slotOf,
  );
  expect(next).toMatchObject({ candidate_skin: null, candidate_skin_dark: "dusk" });
});

test("removing a skin clears only the slots that hold it", () => {
  expect(
    removeCandidateSkin({ candidate_skin: "sakura", candidate_skin_dark: "dusk" }, "dusk"),
  ).toMatchObject({
    candidate_skin: "sakura",
    candidate_skin_dark: null,
  });
  expect(
    removeCandidateSkin({ candidate_skin: "mist", candidate_skin_dark: "mist" }, "mist"),
  ).toMatchObject({
    candidate_skin: null,
    candidate_skin_dark: null,
  });
});

test("a base left by a skin of the other mode follows the host", () => {
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, null, false)).toBe("system");
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, null, true)).toBe("night");
  expect(customDrawnBase({ base: "night" }, null, false)).toBe("night");
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, "paper", false)).toBe(
    "paper",
  );
});
