/** octagram `.gram` 语法模型选择：`none` 不用（默认）；`wanxiang` 万象 LTS（CC-BY-4.0）；`baguwen_word`/`baguwen_compact` 八股文·词级与紧凑（LGPL-3.0）。白霜（GPL-3.0 且构建链许可未声明）不提供。 */
export type GrammarModel = "none" | "wanxiang" | "baguwen_word" | "baguwen_compact" | "baishuang";

/** 本地整句候选来源与神经重排偏好。缺键时 `word_lattice` 与 `neural_keyboard`（整句联想「增强」）都按开读，与 client-core 的默认一致；`neural_desktop` 按关读。 */
export type SentenceAssociationPreferences = {
  word_lattice?: boolean;
  neural_desktop?: boolean;
  neural_keyboard?: boolean;
  show_next_on_duplicate?: boolean;
  grammar_model?: GrammarModel;
};

/** 每个语法模型对应的资源包 id；`none` 不对应包。 */
export const grammarModelPack: Record<Exclude<GrammarModel, "none">, string> = {
  wanxiang: "grammar-model-wanxiang",
  baguwen_word: "grammar-model-baguwen",
  baguwen_compact: "grammar-model-baguwen-compact",
  baishuang: "grammar-model-baishuang",
};

/** 「整句联想」的级别：0 关闭，1 标准（词网格），2 增强（词网格加逐键重排模型，默认）。HarmonyOS 手机「表达」页列出全部三档；桌面输入页只列标准和增强，词网格开关另在「更多选项」里。 */
export type SentenceAssociationLevel = 0 | 1 | 2;

/** 从存储的开关读出级别，与 Android 的 `ExpressionPage.sentenceLevel` 相同：默认值（`word_lattice`、`neural_keyboard` 都开）为「增强」，词网格关闭时不论神经开关如何都是「关闭」。 */
export function sentenceAssociationLevel(
  value: SentenceAssociationPreferences | undefined,
): SentenceAssociationLevel {
  if (!(value?.word_lattice ?? true)) return 0;
  return (value?.neural_keyboard ?? true) ? 2 : 1;
}

/** 桌面输入页「整句联想」只管逐键重排这一个开关：增强为 2、标准为 1，缺键按增强读；词网格的开关不在这一行，所以不读它。 */
export function keyboardRerankLevel(value: SentenceAssociationPreferences | undefined): 1 | 2 {
  return (value?.neural_keyboard ?? true) ? 2 : 1;
}

/** 把级别写回两个开关，其他字段（`neural_desktop`、`show_next_on_duplicate`）保持存储时的值。 */
export function sentenceAssociationWithLevel(
  value: SentenceAssociationPreferences | undefined,
  level: SentenceAssociationLevel,
): SentenceAssociationPreferences {
  return { ...value, word_lattice: level > 0, neural_keyboard: level > 1 };
}

/** 只读取开关与宿主能力的显示状态，不补写缺省值或清除其他设备的选择。 */
export function sentenceAssociationControlState(
  value: SentenceAssociationPreferences | undefined,
  { mobile, neuralKeyboard = true }: { mobile: boolean; neuralKeyboard?: boolean },
) {
  return {
    wordLatticeChecked: value?.word_lattice ?? true,
    neuralChecked: mobile ? (value?.neural_keyboard ?? true) : (value?.neural_desktop ?? false),
    neuralVisible: !mobile || neuralKeyboard,
  };
}
