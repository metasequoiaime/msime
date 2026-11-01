import {
  grammarModelPack,
  sentenceAssociationControlState,
  sentenceAssociationLevel,
  sentenceAssociationWithLevel,
  type GrammarModel,
  type SentenceAssociationLevel,
  type SentenceAssociationPreferences,
} from "./sentence-association-preferences";
import { formatModelBytes } from "../voice/local-model-helpers";
import {
  ResourcePackRow,
  resourcePackStatus,
  resourcePackTitles,
  type ResourcePackId,
  type ResourcePacks,
} from "./resource-packs";
import { SelectRow } from "./select-row";
import { ActionRow } from "./action-row";
import { SwitchRow } from "./switch-row";
import { useState } from "react";

export {
  sentenceAssociationLevel,
  sentenceAssociationWithLevel,
  type SentenceAssociationLevel,
} from "./sentence-association-preferences";

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

export interface SentenceWordLatticeRowProps {
  value: SentenceAssociationPreferences | undefined;
  onChange: (value: SentenceAssociationPreferences) => void;
}

/** 「本地整句联想」开关。关掉会去掉整句候选，几乎没人该动它，所以输入页把它收进「候选与联想」组的「更多选项」，不和神经联想排在一起。 */
export function SentenceWordLatticeRow({ value, onChange }: SentenceWordLatticeRowProps) {
  // 词网格开关的显示状态与宿主形态无关。
  const { wordLatticeChecked } = sentenceAssociationControlState(value, { mobile: false });
  return (
    <SwitchRow
      title="本地整句联想"
      description="把词库组合出的整句加入候选；关闭后仍保留单词候选。"
      checked={wordLatticeChecked}
      onChange={(checked) => onChange({ ...value, word_lattice: checked })}
    />
  );
}

/** 神经联想开关（桌面还带模型下载行）：输入页「候选与联想」组里的一行；「本地整句联想」见 `SentenceWordLatticeRow`。 */
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
  const state = sentenceAssociationControlState(value, { mobile, neuralKeyboard });
  const description = mobile
    ? "使用随包的神经模型重排整句候选；没有模型时保持现有候选。"
    : settledPack && settledPack.state !== "installed"
      ? `使用神经模型重排整句候选，需下载约 ${formatModelBytes(settledPack.size)} 模型；没有模型时保持现有候选。`
      : "使用神经模型重排整句候选；没有模型时保持现有候选。";
  return (
    <>
      {state.neuralVisible && (
        <SwitchRow
          title={mobile ? "键盘神经联想" : "桌面神经联想"}
          description={description}
          checked={state.neuralChecked}
          onChange={(checked) => {
            onChange({
              ...value,
              ...(mobile ? { neural_keyboard: checked } : { neural_desktop: checked }),
            });
            if (!mobile && checked) resourcePacks?.ensure("settled-model");
          }}
        />
      )}
      {!mobile && state.neuralChecked && resourcePacks && (
        <ResourcePackRow packs={resourcePacks} id="settled-model" note="桌面神经联想需要它" />
      )}
      {/* 只作用于模型重排过的整句，所以和神经联想同组；触屏宿主的「整句联想」级别里没有这一项，不显示。 */}
      {!mobile && (
        <SwitchRow
          title="去重后补充次选"
          description="某个模型的首选与已有候选重复时，显示该模型排名最高的下一条不同结果。"
          checked={value?.show_next_on_duplicate ?? false}
          onChange={(checked) => onChange({ ...value, show_next_on_duplicate: checked })}
        />
      )}
      {!mobile && (
        <GrammarModelRow value={value} resourcePacks={resourcePacks} onChange={onChange} />
      )}
    </>
  );
}

/** 语法模型单选及其下载/删除行，只有桌面宿主提供。选中但没装好时显示下载
 * 行（文件校验 SHA-256 后才发布），装好后提供删除入口；删除前先由宿主
 * 释放引擎的共享内存映射。 */
function GrammarModelRow({
  value,
  resourcePacks,
  onChange,
}: {
  value: SentenceAssociationPreferences | undefined;
  resourcePacks?: ResourcePacks;
  onChange: (value: SentenceAssociationPreferences) => void;
}) {
  const [checking, setChecking] = useState(false);
  const [updatePack, setUpdatePack] = useState<ResourcePackId>();
  const [checkError, setCheckError] = useState<string>();
  const model = value?.grammar_model ?? "none";
  const pack = model === "none" ? undefined : (grammarModelPack[model] as ResourcePackId);
  const status = pack ? resourcePackStatus({ statuses: resourcePacks?.statuses }, pack) : undefined;
  const checkUpdates = async () => {
    if (!resourcePacks?.checkGrammarUpdates || checking) return;
    setChecking(true);
    setCheckError(undefined);
    try {
      const updates = await resourcePacks.checkGrammarUpdates();
      setUpdatePack(pack && updates.some((update) => update.pack === pack) ? pack : undefined);
    } catch {
      setCheckError("无法读取官方模型目录，请稍后重试。");
    } finally {
      setChecking(false);
    }
  };
  return (
    <>
      <SelectRow
        title="语法模型"
        description="octagram 语法模型给整句候选加分；模型按需下载，不随安装包。选择后切换一次输入框生效。"
        value={model}
        onChange={(event) =>
          onChange({ ...value, grammar_model: event.target.value as GrammarModel })
        }
      >
        <option value="none">不使用</option>
        <option value="wanxiang">万象 LTS（约 380MB，CC-BY-4.0）</option>
        <option value="baguwen_word">八股文·词级（约 197MB，LGPL-3.0）</option>
        <option value="baguwen_compact">八股文·词级紧凑（约 39MB，LGPL-3.0）</option>
      </SelectRow>
      {pack && resourcePacks && <ResourcePackRow packs={resourcePacks} id={pack} />}
      {pack && resourcePacks?.checkGrammarUpdates && (
        <ActionRow
          title="模型更新"
          description="不会自动联网；点击后才检查官方模型目录。"
          detail={
            checkError ?? (updatePack === pack ? "发现可选更新；当前模型仍可离线使用。" : undefined)
          }
          action={checkUpdates}
          label={checking ? "检查中…" : "检查更新"}
          ariaLabel={"检查" + resourcePackTitles[pack] + "更新"}
        />
      )}
      {pack && updatePack === pack && status?.state === "installed" && resourcePacks && (
        <ActionRow
          title="下载模型更新"
          description="下载并完整校验后替换当前模型；下载失败保留旧模型。"
          detail={resourcePacks.errors[pack]}
          action={() => resourcePacks.install(pack)}
          label={resourcePacks.progress[pack] ? "下载中…" : "下载更新"}
          disabled={Boolean(resourcePacks.progress[pack])}
          ariaLabel={"下载" + resourcePackTitles[pack] + "更新"}
        />
      )}
      {pack && status?.state === "installed" && resourcePacks && (
        <ActionRow
          title={resourcePackTitles[pack]}
          description="删除后切换输入框即可退回无语法模型的候选行为。"
          detail={resourcePacks.errors[pack] ?? "已安装"}
          action={() => resourcePacks.remove(pack)}
          label="删除"
          ariaLabel={`删除${resourcePackTitles[pack]}`}
        />
      )}
    </>
  );
}

const sentenceAssociationLevelLabels: readonly [SentenceAssociationLevel, string][] = [
  [0, "关闭"],
  [1, "标准"],
  [2, "增强"],
];

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
