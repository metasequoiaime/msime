import { EditorTraits } from "./EditorPolicy";

/** 由系统运行时提供的编辑框类型，不依赖声明文件中的数值。 */
export interface EditorPatterns {
  readonly PATTERN_TEXT: number;
  readonly PATTERN_NULL: number;
  readonly PATTERN_EMAIL: number;
  readonly PATTERN_URI: number;
  readonly PATTERN_USER_NAME: number;
  readonly PATTERN_PASSWORD: number;
  readonly PATTERN_PASSWORD_SCREEN_LOCK: number;
  readonly PATTERN_PASSWORD_NUMBER: number;
  readonly PATTERN_NEW_PASSWORD: number;
  readonly PATTERN_ONE_TIME_CODE: number;
  readonly PATTERN_NUMBER: number;
  readonly PATTERN_PHONE: number;
  readonly PATTERN_NUMBER_DECIMAL: number;
}

export class EditorPatternPolicy {
  /** 多行普通文本和单行普通文本一样进入组字；未知类型仍不进入组字。 */
  static traitsFor(pattern: number, types: EditorPatterns, multiline: number): EditorTraits {
    const password: boolean = pattern === types.PATTERN_PASSWORD
      || pattern === types.PATTERN_PASSWORD_SCREEN_LOCK
      || pattern === types.PATTERN_PASSWORD_NUMBER
      || pattern === types.PATTERN_NEW_PASSWORD;
    return {
      text: pattern === types.PATTERN_TEXT
        || pattern === types.PATTERN_NULL
        || pattern === multiline
        || pattern === types.PATTERN_EMAIL
        || pattern === types.PATTERN_URI
        || pattern === types.PATTERN_USER_NAME,
      password: password,
      uri: pattern === types.PATTERN_URI,
      email: pattern === types.PATTERN_EMAIL,
      noSuggestions: pattern === types.PATTERN_ONE_TIME_CODE
        || pattern === types.PATTERN_USER_NAME,
      digits: pattern === types.PATTERN_NUMBER
        || pattern === types.PATTERN_PHONE
        || pattern === types.PATTERN_NUMBER_DECIMAL,
    };
  }
}
