import { expect, test } from "vitest";
import { communityDestinationView } from "@msime/ui";

test.each([
  ["all", { category: "skin", scope: "", initialMine: false }],
  ["published-skins", { category: "skin", scope: "", initialMine: true }],
  ["published-dictionary", { category: "dictionary", scope: "mine", initialMine: false }],
  ["saved-reply", { category: "reply", scope: "saved", initialMine: false }],
] as const)("maps %s to its initial community view", (destination, expected) => {
  expect(communityDestinationView(destination)).toEqual(expected);
});
