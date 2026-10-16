/** 对应 `client-core::preferences::InputScheme`。 */
export type InputScheme =
  | "quanpin"
  | "shuangpin"
  | "wubi"
  | "japanese"
  | "korean"
  | "cantonese"
  | "zhuyin"
  | "vietnamese"
  | "tibetan"
  | "stroke";
/** 对应 `client-core::preferences::ChineseScheme`：选日文、韩文、越南文或藏文后要回到的中文方案。 */
export type ChineseScheme = "quanpin" | "shuangpin" | "wubi" | "cantonese" | "zhuyin" | "stroke";
