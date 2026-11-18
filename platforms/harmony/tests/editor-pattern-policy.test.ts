import { EditorPatternPolicy, EditorPatterns } from "../entry/src/main/ets/keyboard/input/EditorPatternPolicy";
import { EditorPolicy } from "../entry/src/main/ets/keyboard/input/EditorPolicy";
import { HardwareKeyRouter, HardwareKeyAction } from "../entry/src/main/ets/keyboard/HardwareKeyRouter";

// 使用合成常量，确认策略读取传入的系统值，不依赖枚举在运行时的固定编号。
const types: EditorPatterns = {
  PATTERN_TEXT: 100, PATTERN_NULL: 101, PATTERN_EMAIL: 102, PATTERN_URI: 103,
  PATTERN_USER_NAME: 104, PATTERN_PASSWORD: 105, PATTERN_PASSWORD_SCREEN_LOCK: 106,
  PATTERN_PASSWORD_NUMBER: 107, PATTERN_NEW_PASSWORD: 108, PATTERN_ONE_TIME_CODE: 109,
  PATTERN_NUMBER: 110, PATTERN_PHONE: 111, PATTERN_NUMBER_DECIMAL: 112,
};
const multiline = 113;
const traits = (pattern: number) => EditorPatternPolicy.traitsFor(pattern, types, multiline);
const check = (condition: boolean, message: string): void => { if (!condition) throw new Error(message); };
check(EditorPolicy.useEngine(traits(multiline)), "多行文本应允许组字");
check(!EditorPolicy.prefersLatin(traits(multiline)), "多行文本不应强制英文");
const letter = { keyCode: 2017, unicodeChar: 97, ctrlKey: false, altKey: false, logoKey: false, shiftKey: false };
check(HardwareKeyRouter.route(letter, false, EditorPolicy.useEngine(traits(multiline))).action === HardwareKeyAction.COMPOSE,
  "多行框中的字母应进入组字路由");
check(EditorPolicy.useEngine(traits(types.PATTERN_TEXT)), "普通文本应允许组字");
check(EditorPolicy.useEngine(traits(types.PATTERN_NULL)), "默认文本应允许组字");
for (const value of [types.PATTERN_PASSWORD, types.PATTERN_PASSWORD_SCREEN_LOCK,
  types.PATTERN_PASSWORD_NUMBER, types.PATTERN_NEW_PASSWORD, types.PATTERN_ONE_TIME_CODE,
  types.PATTERN_NUMBER, types.PATTERN_PHONE, types.PATTERN_NUMBER_DECIMAL, 999]) {
  check(!EditorPolicy.useEngine(traits(value)), "密码、验证码、数字及未知类型不应组字");
}
check(EditorPolicy.prefersLatin(traits(types.PATTERN_EMAIL)), "邮箱沿用英文模式");
check(EditorPolicy.prefersLatin(traits(types.PATTERN_URI)), "网址沿用英文模式");
check(EditorPolicy.prefersLatin(traits(types.PATTERN_USER_NAME)), "用户名沿用英文模式");
console.log("editor pattern policy tests passed");
