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

test("an unknown previous skin is overwritten rather than moved", () => {
  const next = applyCandidateSkin({ candidate_skin: "removed-skin" }, "sakura", "paper", slotOf);
  expect(next.candidate_skin).toBe("sakura");
  expect(next.candidate_skin_dark).toBeUndefined();
  // system 底的旧皮肤确知能在深色模式画，照样挪过去。
  const moved = applyCandidateSkin({ candidate_skin: "mist" }, "sakura", "paper", slotOf);
  expect(moved).toMatchObject({ candidate_skin: "sakura", candidate_skin_dark: "mist" });
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

test("removing the last skin does not leave its base fixing both modes", () => {
  // 先用浅色皮肤、再用深色皮肤，底是深色皮肤的 night。
  const both = applyCandidateSkin(
    applyCandidateSkin(undefined, "sakura", "paper", slotOf),
    "starry",
    "night",
    slotOf,
  );
  expect(both).toMatchObject({
    base: "night",
    candidate_skin: "sakura",
    candidate_skin_dark: "starry",
  });
  // 还剩一款时底不动：深色模式没有能画的包时仍画 night，浅色模式照旧画 sakura。
  const darkOnly = removeCandidateSkin(both, "sakura");
  expect(darkOnly).toMatchObject({
    base: "night",
    candidate_skin: null,
    candidate_skin_dark: "starry",
  });
  expect(customDrawnBase(darkOnly, null, false)).toBe("system");
  // 最后一款取下后底回到 system，浅色模式不会变成深色的 night。
  const none = removeCandidateSkin(darkOnly, "starry");
  expect(none).toMatchObject({ base: "system", candidate_skin: null, candidate_skin_dark: null });
  expect(customDrawnBase(none, null, false)).toBe("system");
  expect(customDrawnBase(none, null, true)).toBe("system");
  // 没放在任何槽位里的包，取下时什么都不改，取色器用的底也不动。
  expect(removeCandidateSkin({ base: "ink" }, "starry")).toEqual({ base: "ink" });
});

test("a base left by a skin of the other mode follows the host", () => {
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, null, false)).toBe("system");
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, null, true)).toBe("night");
  expect(customDrawnBase({ base: "night" }, null, false)).toBe("night");
  expect(customDrawnBase({ base: "night", candidate_skin: "starry" }, "paper", false)).toBe(
    "paper",
  );
});
