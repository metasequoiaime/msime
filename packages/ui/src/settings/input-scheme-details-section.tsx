import { Row } from "../core/platform-controls";
import type { InputScheme, VietnamesePreferences } from "../index";
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

export type InputSchemeDetailsScheme = InputScheme;
export type ShuangpinProfile = "xiaohe" | "ziranma" | "shoudao" | "microsoft";
export type WubiProfile = "wubi86" | "wubi98";

/** 双拼方案的选项。这里和下面的五笔方案都写成返回 Fragment 的普通函数，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
function shuangpinProfileOptions() {
  return (
    <>
      <option value="xiaohe">小鹤双拼</option>
      <option value="ziranma">自然码双拼</option>
      <option value="shoudao">首道双拼</option>
      <option value="microsoft">微软双拼</option>
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
  macos: boolean;
  hasTouchKeyboardSchemes: boolean;
  /** 触屏宿主启用了五笔键盘：触屏只有一个五笔键盘，86 还是 98 仍在这里选。 */
  touchKeyboardHasWubi?: boolean;
  macosShuangpinKeymap?: boolean;
  onShuangpinProfileChange: (profile: ShuangpinProfile) => void;
  onWubiProfileChange?: (profile: WubiProfile) => void;
  onMacosShuangpinKeymapChange?: (enabled: boolean) => void;
  /** The document's Vietnamese options; absent means the defaults, Telex with modern tone placement. */
  vietnamese?: VietnamesePreferences;
  onVietnameseChange?: (vietnamese: VietnamesePreferences) => void;
}

const cantoneseDescription = "按不带声调的粤拼输入，用 ' 分隔音节，数字键选词，候选为繁体字词";
const zhuyinDescription =
  "按大千键位输入注音符号，空格为一声，6 3 4 7 为二、三、四声和轻声，候选为繁体字词";
const strokeDescription =
  "按 h s p n z 依次输入横、竖、撇、点、折，x 代替不确定的一笔，数字键选词，候选为笔顺以此开头的单字";
const vietnameseDescription = "Telex 用字母、VNI 用数字标注声调和变音，Esc 恢复原始按键";
const tibetanDescription =
  "按 EWTS 威利转写输入，区分大小写；空格加音节点 ་ 上屏，/ 加垂符 ། 上屏，回车只上屏藏文，Esc 恢复原始按键";
const toneStyleDescription = "新式把声调标在主元音上（hoà），旧式按传统位置标注（hòa）";

/** Shared scheme-specific controls for desktop input settings. */
export function InputSchemeDetailsSection({
  scheme,
  shuangpinProfile,
  wubiProfile = "wubi86",
  macos,
  hasTouchKeyboardSchemes,
  touchKeyboardHasWubi = false,
  macosShuangpinKeymap,
  onShuangpinProfileChange,
  onWubiProfileChange,
  onMacosShuangpinKeymapChange,
  vietnamese,
  onVietnameseChange,
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
  const inputMethod = vietnamese?.input_method ?? "telex";
  const toneStyle = vietnamese?.tone_style ?? "modern";
  const changeVietnamese = (patch: VietnamesePreferences) =>
    onVietnameseChange?.({ input_method: inputMethod, tone_style: toneStyle, ...patch });

  return (
    <>
      <SelectRow
        title="双拼方案"
        hidden={hideChineseSchemeOptions}
        disabled={macos && scheme !== "shuangpin"}
        value={shuangpinProfile}
        onChange={(event) => onShuangpinProfileChange(event.target.value as ShuangpinProfile)}
      >
        {shuangpinProfileOptions()}
      </SelectRow>
      {macosShuangpinKeymap !== undefined && (
        <SwitchRow
          title="输入时显示双拼键位提示"
          description="双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。"
          hidden={hasTouchKeyboardSchemes || scheme !== "shuangpin"}
          checked={macosShuangpinKeymap}
          onChange={onMacosShuangpinKeymapChange ?? (() => {})}
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
      {/* 日语、韩语各只有一个方案，选择器改不了任何东西，只在对应方案下作为说明出现。 */}
      <SegmentedRow
        title="日语方案"
        description="直接输入罗马音，提供平假名、片假名及日语词库候选"
        hidden={hasTouchKeyboardSchemes || scheme !== "japanese"}
        options={japaneseInputSchemeOptions}
        value="romaji"
        onChange={() => {}}
      />
      <SegmentedRow
        title="韩语方案"
        description="按两套式（두벌식）键位输入韩文字母，自动拼成音节，标点为半角"
        hidden={hasTouchKeyboardSchemes || scheme !== "korean"}
        options={koreanInputSchemeOptions}
        value="dubeolsik"
        onChange={() => {}}
      />
      <SegmentedRow
        title="粤拼方案"
        description={cantoneseDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "cantonese"}
        options={cantoneseInputSchemeOptions}
        value="jyutping"
        onChange={() => {}}
      />
      <SegmentedRow
        title="注音键盘"
        description={zhuyinDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "zhuyin"}
        options={zhuyinLayoutOptions}
        value="dachen"
        onChange={() => {}}
      />
      <SegmentedRow
        title="笔画方案"
        description={strokeDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "stroke"}
        options={strokeLayoutOptions}
        value="hspnz"
        onChange={() => {}}
      />
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
      <SegmentedRow
        title="藏文方案"
        description={tibetanDescription}
        hidden={hasTouchKeyboardSchemes || scheme !== "tibetan"}
        options={tibetanInputSchemeOptions}
        value="ewts"
        onChange={() => {}}
      />
    </>
  );
}
