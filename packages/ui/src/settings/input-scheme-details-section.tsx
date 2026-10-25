import type { WubiProfile } from "./wubi-preferences";
import type { InputScheme } from "../core/input-scheme";
import { LinkRow, Row } from "../core/platform-controls";

import { resolveVietnamesePreferences, type VietnamesePreferences } from "./vietnamese-preferences";
import {
  koreanLayoutDescription,
  resolveKoreanLayout,
  type KoreanPreferences,
} from "./korean-preferences";
import {
  cantoneseInputSchemeOptions,
  japaneseInputSchemeOptions,
  koreanInputSchemeOptions,
  strokeLayoutOptions,
  tibetanInputSchemeOptions,
  vietnameseInputMethodOptions,
  vietnameseToneStyleOptions,
  zhuyinLayoutOptions,
} from "./input-scheme-options";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";
import { SegmentedRow } from "./segmented-row";
import { PreeditStyleRow } from "./preedit-style-row";

export type InputSchemeDetailsScheme = InputScheme;
export type ShuangpinProfile =
  | "xiaohe"
  | "ziranma"
  | "shoudao"
  | "microsoft"
  | "smart_abc"
  | "custom";
export type { WubiProfile } from "./wubi-preferences";

/**
 * 双拼方案的选项。这里和下面的五笔方案都写成返回 Fragment 的普通函数，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。
 *
 * 「自定义双拼」在宿主能编辑它（`editable`）时列出；文档里已经是它而宿主编辑不了时也列出，但不能选，否则受控的选择框找不到这一项，会把第一项显示成当前方案。
 */
function shuangpinProfileOptions(editable: boolean, current: ShuangpinProfile) {
  return (
    <>
      <option value="xiaohe">小鹤双拼</option>
      <option value="ziranma">自然码双拼</option>
      <option value="shoudao">首道双拼</option>
      <option value="microsoft">微软双拼</option>
      <option value="smart_abc">智能 ABC 双拼</option>
      {(editable || current === "custom") && (
        <option value="custom" disabled={!editable}>
          自定义双拼
        </option>
      )}
    </>
  );
}

function wubiProfileOptions() {
  return (
    <>
      <option value="wubi86">86 五笔</option>
      <option value="wubi98">98 五笔</option>
    </>
  );
}

export interface InputSchemeDetailsSectionProps {
  scheme: InputSchemeDetailsScheme;
  shuangpinProfile: ShuangpinProfile;
  /** 缺省为 86 五笔。 */
  wubiProfile?: WubiProfile;
  hasTouchKeyboardSchemes: boolean;
  /** 触屏宿主启用了五笔键盘：触屏只有一个五笔键盘，86 还是 98 仍在这里选。 */
  touchKeyboardHasWubi?: boolean;
  /** 「输入时显示双拼键位提示」的当前值；宿主不画双拼键位图时不传，这一行不出现。 */
  shuangpinKeymapHint?: boolean;
  /** Linux 宿主：键位图只画给报根窗口坐标光标的 X11 客户端（`platforms/linux/src/overlay/ShuangpinKeymapX11Surface.h`），描述照实说明在哪里不显示。 */
  linux?: boolean;
  /** 「双拼预编辑」的当前样式（保留原始按键或显示拼音分词）；宿主不画双拼预编辑（`HostCapabilities.shuangpin_preedit` 为假）或在触屏宿主上时不传，这一行不出现。 */
  shuangpinPreedit?: "raw" | "pinyin";
  onShuangpinProfileChange: (profile: ShuangpinProfile) => void;
  /** 宿主能校验自定义双拼表时传入，打开自定义双拼的编辑页。有它时方案里多一项「自定义双拼」：选它先打开编辑页，在那里保存一张能用的表才切过去，与 MCP 不让选一张不能用的表相同；已经是自定义双拼时下面多一行「编辑自定义键位」。 */
  onEditCustomShuangpin?: () => void;
  onWubiProfileChange?: (profile: WubiProfile) => void;
  onShuangpinKeymapHintChange?: (enabled: boolean) => void;
  /** 选了双拼预编辑的样式：`true` 为保留原始按键，写进共享偏好 `shuangpin_preedit_uses_raw`。 */
  onShuangpinPreeditChange?: (usesRaw: boolean) => void;
  /** The document's Vietnamese options; absent means the defaults, Telex with modern tone placement. */
  vietnamese?: VietnamesePreferences;
  onVietnameseChange?: (vietnamese: VietnamesePreferences) => void;
  korean?: KoreanPreferences;
  koreanRomaja?: boolean;
  onKoreanChange?: (korean: KoreanPreferences) => void;
}

const cantoneseDescription = "按不带声调的粤拼输入，用 ' 分隔音节，数字键选词，候选为繁体字词";
const zhuyinDescription =
  "按大千键位输入注音符号，空格为一声，6 3 4 7 为二、三、四声和轻声，候选为繁体字词";
const strokeDescription =
  "按 h s p n z 依次输入横、竖、撇、点、折，x 代替不确定的一笔，数字键选词，候选为笔顺以此开头的单字";
const vietnameseDescription = "Telex 用字母、VNI 用数字标注声调和变音，Esc 恢复原始按键";
const tibetanDescription =
  "按 EWTS 威利转写输入，区分大小写；空格加音节点 ་ 上屏，/ 加垂符 ། 上屏，回车只上屏藏文，Esc 恢复原始按键";
const keymapDescription = "双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。";
// IBus 只在纯 X11 会话里画（Wayland 会话里分不出客户端是不是 Xwayland），Fcitx5 按输入上下文画给 X11 客户端，Wayland 原生应用的光标坐标不能用来摆放。
const linuxKeymapDescription = `${keymapDescription}Linux 只为 X11 应用显示：Wayland 桌面上 IBus 不显示，Fcitx5 只为经 Xwayland 运行的应用显示。`;
const toneStyleDescription = "新式把声调标在主元音上（hoà），旧式按传统位置标注（hòa）";

/** Shared scheme-specific controls for desktop input settings. */
export function InputSchemeDetailsSection({
  scheme,
  shuangpinProfile,
  wubiProfile = "wubi86",
  hasTouchKeyboardSchemes,
  touchKeyboardHasWubi = false,
  shuangpinKeymapHint,
  linux = false,
  shuangpinPreedit,
  onShuangpinProfileChange,
  onEditCustomShuangpin,
  onWubiProfileChange,
  onShuangpinKeymapHintChange,
  onShuangpinPreeditChange,
  vietnamese,
  onVietnameseChange,
  korean,
  koreanRomaja = false,
  onKoreanChange,
}: InputSchemeDetailsSectionProps) {
  // 双拼方案和五笔方案只涉及拼音和五笔；粤拼、注音、越南文、藏文和笔画各有自己的行。
  const hideChineseSchemeOptions =
    hasTouchKeyboardSchemes ||
    scheme === "japanese" ||
    scheme === "korean" ||
    scheme === "cantonese" ||
    scheme === "zhuyin" ||
    scheme === "vietnamese" ||
    scheme === "tibetan" ||
    scheme === "stroke";
  const { input_method: inputMethod, tone_style: toneStyle } =
    resolveVietnamesePreferences(vietnamese);
  const changeVietnamese = (patch: VietnamesePreferences) =>
    onVietnameseChange?.({ input_method: inputMethod, tone_style: toneStyle, ...patch });
  // 双拼的各行只在双拼方案下出现：其他方案下它们改不了正在用的方案，原先灰着显示的双拼方案也只是多出来的一行。
  const hideShuangpinRows = hideChineseSchemeOptions || scheme !== "shuangpin";
  // 单一布局只展示输入说明；韩语在声明能力的桌面宿主上另给布局选择。
  const schemeNote = (
    shown: InputSchemeDetailsScheme,
    title: string,
    options: readonly { label: string }[],
    description: string,
  ) => (
    <Row
      title={title}
      description={`${options[0].label}：${description}`}
      hidden={hasTouchKeyboardSchemes || scheme !== shown}
    />
  );

  return (
    <>
      <SelectRow
        title="双拼方案"
        hidden={hideShuangpinRows}
        value={shuangpinProfile}
        onChange={(event) => {
          const profile = event.target.value as ShuangpinProfile;
          if (profile === "custom" && onEditCustomShuangpin) onEditCustomShuangpin();
          else onShuangpinProfileChange(profile);
        }}
      >
        {shuangpinProfileOptions(Boolean(onEditCustomShuangpin), shuangpinProfile)}
      </SelectRow>
      {onEditCustomShuangpin && shuangpinProfile === "custom" && (
        <div hidden={hideShuangpinRows}>
          <LinkRow
            title="编辑自定义键位"
            description="改键、导入或导出这张双拼表。"
            onClick={onEditCustomShuangpin}
          />
        </div>
      )}
      {shuangpinKeymapHint !== undefined && (
        <SwitchRow
          title="输入时显示双拼键位提示"
          description={linux ? linuxKeymapDescription : keymapDescription}
          hidden={hideShuangpinRows}
          checked={shuangpinKeymapHint}
          onChange={onShuangpinKeymapHintChange ?? (() => {})}
        />
      )}
      {/* 双拼预编辑以前在「候选窗口」页，对所有人显示，说明却写着只在双拼下生效；放到这里只在双拼方案下出现。 */}
      {shuangpinPreedit !== undefined && !hideShuangpinRows && (
        <PreeditStyleRow
          title="双拼预编辑"
          description="保留原始双拼按键，或显示展开后的拼音分词。"
          mode="shuangpin"
          value={shuangpinPreedit}
          onChange={(value) => onShuangpinPreeditChange?.(value === "raw")}
        />
      )}
      {/* 五笔方案在 86 与 98 码表之间切换，个人词条和学习记录按版本分开存；触屏宿主没有方案选择器，启用了五笔键盘时也在这里选。 */}
      <SelectRow
        title="五笔方案"
        hidden={hasTouchKeyboardSchemes ? !touchKeyboardHasWubi : scheme !== "wubi"}
        value={wubiProfile}
        onChange={(event) => onWubiProfileChange?.(event.target.value as WubiProfile)}
      >
        {wubiProfileOptions()}
      </SelectRow>
      {schemeNote(
        "japanese",
        "日语方案",
        japaneseInputSchemeOptions,
        "直接输入罗马音，提供平假名、片假名及日语词库候选",
      )}
      {koreanRomaja ? (
        <SegmentedRow
          title="韩语布局"
          description={koreanLayoutDescription}
          hidden={hasTouchKeyboardSchemes || scheme !== "korean"}
          options={koreanInputSchemeOptions}
          value={resolveKoreanLayout(korean)}
          onChange={(layout) => onKoreanChange?.({ ...korean, layout })}
        />
      ) : (
        schemeNote(
          "korean",
          "韩语方案",
          koreanInputSchemeOptions,
          "按两套式（두벌식）键位输入韩文字母，自动拼成音节，标点为半角",
        )
      )}
      {schemeNote("cantonese", "粤拼方案", cantoneseInputSchemeOptions, cantoneseDescription)}
      {schemeNote("zhuyin", "注音键盘", zhuyinLayoutOptions, zhuyinDescription)}
      {schemeNote("stroke", "笔画方案", strokeLayoutOptions, strokeDescription)}
      <SegmentedRow
        title="越南语方案"
        description={vietnameseDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
        options={vietnameseInputMethodOptions}
        value={inputMethod}
        onChange={(input_method) => changeVietnamese({ input_method })}
      />
      <SegmentedRow
        title="声调位置"
        description={toneStyleDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
        options={vietnameseToneStyleOptions}
        value={toneStyle}
        onChange={(tone_style) => changeVietnamese({ tone_style })}
      />
      {schemeNote("tibetan", "藏文方案", tibetanInputSchemeOptions, tibetanDescription)}
    </>
  );
}
