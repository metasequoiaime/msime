import { expect, test } from "vitest";
import { wubiControlState } from "@msime/ui";

test("五笔缺键时按版本决定混输，其余两项默认开启且不补写偏好", () => {
  const preferences = Object.freeze({});
  expect(wubiControlState(preferences)).toEqual({
    wubi_mixed_pinyin: false,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
  });
  expect(wubiControlState(preferences, true)).toEqual({
    wubi_mixed_pinyin: true,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
  });
  expect(preferences).toEqual({});
});

test("显式布尔选择优先于版本和旧文档缺省", () => {
  const disabled = Object.freeze({
    wubi_mixed_pinyin: false,
    wubi_code_hint: false,
    wubi_auto_commit_unique: false,
  });
  expect(wubiControlState(disabled, true)).toEqual(disabled);
  const enabled = Object.freeze({
    wubi_mixed_pinyin: true,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
  });
  expect(wubiControlState(enabled, false)).toEqual(enabled);
});
