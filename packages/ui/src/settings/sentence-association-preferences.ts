/** 本地整句候选来源与可选的神经重排偏好。 */
export type SentenceAssociationPreferences = {
  word_lattice?: boolean;
  neural_desktop?: boolean;
  neural_keyboard?: boolean;
  show_next_on_duplicate?: boolean;
};

/** HarmonyOS 手机「表达」页上「整句联想」的三个级别：0 关闭，1 标准（词网格），2 增强（词网格加键盘神经模型）。 */
export type SentenceAssociationLevel = 0 | 1 | 2;

/** 从存储的开关读出级别，与 Android 的 `ExpressionPage.sentenceLevel` 相同：默认值（`word_lattice` 开、`neural_keyboard` 关）为「标准」，词网格关闭时不论神经开关如何都是「关闭」。 */
export function sentenceAssociationLevel(
  value: SentenceAssociationPreferences | undefined,
): SentenceAssociationLevel {
  if (!(value?.word_lattice ?? true)) return 0;
  return value?.neural_keyboard ? 2 : 1;
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
    neuralChecked: (mobile ? value?.neural_keyboard : value?.neural_desktop) ?? false,
    neuralVisible: !mobile || neuralKeyboard,
  };
}
