import { expect, test } from "vitest";
import { schemeTitle } from "@msime/ui";

test("labels every supported input scheme", () => {
  expect(schemeTitle("quanpin")).toBe("全拼");
  expect(schemeTitle("shuangpin")).toBe("双拼");
  expect(schemeTitle("wubi")).toBe("五笔");
  expect(schemeTitle("japanese")).toBe("日语");
  expect(schemeTitle("korean")).toBe("韩语");
  expect(schemeTitle("cantonese")).toBe("粤拼");
  expect(schemeTitle("zhuyin")).toBe("注音");
  expect(schemeTitle("vietnamese")).toBe("越南语");
  expect(schemeTitle("tibetan")).toBe("藏文");
  expect(schemeTitle("stroke")).toBe("笔画");
});
