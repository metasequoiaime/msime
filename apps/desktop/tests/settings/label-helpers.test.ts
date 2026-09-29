import { expect, test } from "vitest";
import { schemeTitle } from "@msime/ui";

test("labels every supported input scheme", () => {
  expect(schemeTitle("quanpin")).toBe("全拼");
  expect(schemeTitle("shuangpin")).toBe("双拼");
  expect(schemeTitle("wubi")).toBe("五笔");
  expect(schemeTitle("japanese")).toBe("日语");
});
