import type { SentenceAssociationPreferences } from "../index";
import { formatModelBytes } from "../voice/local-model-helpers";
import { ResourcePackRow, resourcePackStatus, type ResourcePacks } from "./resource-packs";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

export interface SentenceAssociationSectionProps {
  value: SentenceAssociationPreferences | undefined;
  /** 触屏宿主写 `neural_keyboard`，桌面宿主写 `neural_desktop`，行名也随之改为「键盘」或「桌面」。 */
  mobile: boolean;
  /** 本版本带键盘神经联想的模型（`HostCapabilities.edition.neural_keyboard`）；为 false 时触屏宿主不列出神经联想开关。缺省为 true。 */
  neuralKeyboard?: boolean;
  /** 桌面宿主的资源包下载服务。桌面神经联想的模型（`settled-model`）不随包时由它下载：打开开关时开始下载，开关下面显示下载进度或失败原因。 */
  resourcePacks?: ResourcePacks;
  onChange: (value: SentenceAssociationPreferences) => void;
}

/** 本地整句联想与神经联想两个开关：输入页「候选与联想」组里的两行。 */
export function SentenceAssociationSection({
  value,
  mobile,
  neuralKeyboard = true,
  resourcePacks,
  onChange,
}: SentenceAssociationSectionProps) {
  // 宿主没列出 `settled-model` 时（随包带着模型，或者不提供下载服务）没有什么要下载。
  const settledPack =
    !mobile && resourcePacks ? resourcePackStatus(resourcePacks, "settled-model") : undefined;
  const neuralDesktop = value?.neural_desktop ?? false;
  const description = mobile
    ? "使用随包的神经模型重排整句候选；没有模型时保持现有候选。"
    : settledPack && settledPack.state !== "installed"
      ? `使用神经模型重排整句候选，需下载约 ${formatModelBytes(settledPack.size)} 模型；没有模型时保持现有候选。`
      : "使用神经模型重排整句候选；没有模型时保持现有候选。";
  return (
    <>
      <SwitchRow
        title="本地整句联想"
        description="把词库组合出的整句加入候选；关闭后仍保留单词候选。"
        checked={value?.word_lattice ?? true}
        onChange={(checked) => onChange({ ...value, word_lattice: checked })}
      />
      {(!mobile || neuralKeyboard) && (
        <SwitchRow
          title={mobile ? "键盘神经联想" : "桌面神经联想"}
          description={description}
          checked={mobile ? (value?.neural_keyboard ?? false) : neuralDesktop}
          onChange={(checked) => {
            onChange({
              ...value,
              ...(mobile ? { neural_keyboard: checked } : { neural_desktop: checked }),
            });
            if (!mobile && checked) resourcePacks?.ensure("settled-model");
          }}
        />
      )}
      {!mobile && neuralDesktop && resourcePacks && (
        <ResourcePackRow packs={resourcePacks} id="settled-model" note="桌面神经联想需要它" />
      )}
    </>
  );
}

/** HarmonyOS 手机「表达」页上「整句联想」的三个级别：0 关闭，1 标准（词网格），2 增强（词网格加键盘神经模型）。 */
export type SentenceAssociationLevel = 0 | 1 | 2;

const sentenceAssociationLevelLabels: readonly [SentenceAssociationLevel, string][] = [
  [0, "关闭"],
  [1, "标准"],
  [2, "增强"],
];

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

export interface SentenceAssociationLevelRowProps {
  value: SentenceAssociationPreferences | undefined;
  /** 本版本是否附带键盘神经模型（`HostCapabilities.edition.neural_keyboard`）；为 false 时不提供「增强」级别。默认为 true。 */
  neuralKeyboard?: boolean;
  onChange: (value: SentenceAssociationPreferences) => void;
}

/** 「整句联想」级别选择：HarmonyOS 手机「表达」页「智能」分组中的一行，替代其他平台「输入」页上显示的两个开关。 */
export function SentenceAssociationLevelRow({
  value,
  neuralKeyboard = true,
  onChange,
}: SentenceAssociationLevelRowProps) {
  // 没有键盘模型的版本没有「增强」可显示；`host-api` 在那里保持 `neural_keyboard` 关闭，所以存储的「增强」显示为它实际运行的「标准」。
  const level = Math.min(sentenceAssociationLevel(value), neuralKeyboard ? 2 : 1);
  return (
    <SelectRow
      title="整句联想"
      description="更准确，但更耗电"
      value={String(level)}
      onChange={(event) =>
        onChange(
          sentenceAssociationWithLevel(
            value,
            Number(event.target.value) as SentenceAssociationLevel,
          ),
        )
      }
    >
      {sentenceAssociationLevelLabels
        .filter(([option]) => neuralKeyboard || option < 2)
        .map(([option, label]) => (
          <option key={option} value={String(option)}>
            {label}
          </option>
        ))}
    </SelectRow>
  );
}
