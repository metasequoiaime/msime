import { expect, test } from "vitest";
import { helpcodeSchemaOptions } from "@msime/ui";

test("helpcode schema options keep the shared order and labels", () => {
  expect(helpcodeSchemaOptions).toEqual([
    ["lantian", "蓝天小雨点"],
    ["ziranma", "自然码"],
    ["shouyou2_0", "首右2.0"],
    ["shouyouplus", "首右plus"],
    ["xiaohe", "小鹤"],
    ["jiajia", "加加"],
    ["wubi86", "五笔 86"],
  ]);
});
