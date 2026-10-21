import { expect, test } from "vitest";
import { wubiControlState } from "@msime/ui";

test("五笔缺键时按版本决定混输，其余四项沿用各自出厂值且不补写偏好", () => {
  const preferences = Object.freeze({});
  expect(wubiControlState(preferences)).toEqual({
    wubi_mixed_pinyin: false,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
    wubi_z_wildcard: false,
    wubi_fifth_code_top_commit: true,
  });
  expect(wubiControlState(preferences, true)).toEqual({
    wubi_mixed_pinyin: true,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
    wubi_z_wildcard: false,
    wubi_fifth_code_top_commit: true,
  });
  expect(preferences).toEqual({});
});

test("显式布尔选择优先于版本和旧文档缺省", () => {
  const disabled = Object.freeze({
    wubi_mixed_pinyin: false,
    wubi_code_hint: false,
    wubi_auto_commit_unique: false,
    wubi_z_wildcard: false,
    wubi_fifth_code_top_commit: false,
  });
  expect(wubiControlState(disabled, true)).toEqual(disabled);
  const enabled = Object.freeze({
    wubi_mixed_pinyin: true,
    wubi_code_hint: true,
    wubi_auto_commit_unique: true,
    wubi_z_wildcard: true,
    wubi_fifth_code_top_commit: true,
  });
  expect(wubiControlState(enabled, false)).toEqual(enabled);
});
