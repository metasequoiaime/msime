import { Row } from "../core/platform-controls";
import type { InputScheme, VietnamesePreferences } from "../index";
import {
  cantoneseInputSchemeOptions,
  japaneseInputSchemeOptions,
  koreanInputSchemeOptions,
  vietnameseInputMethodOptions,
  vietnameseToneStyleOptions,
  zhuyinLayoutOptions,
} from "./input-scheme-options";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";
import { SegmentedRow } from "./segmented-row";

export type InputSchemeDetailsScheme = InputScheme;
export type ShuangpinProfile = "xiaohe" | "ziranma" | "shoudao" | "microsoft";

function ShuangpinProfileOptions() {
  return (
    <>
      <option value="xiaohe">小鹤双拼</option>
      <option value="ziranma">自然码双拼</option>
      <option value="shoudao">首道双拼</option>
      <option value="microsoft">微软双拼</option>
    </>
  );
}

function WubiSchemeOption() {
  return <option value="wubi86">86 五笔</option>;
}

export interface InputSchemeDetailsSectionProps {
  scheme: InputSchemeDetailsScheme;
  shuangpinProfile: ShuangpinProfile;
  macos: boolean;
  hasTouchKeyboardSchemes: boolean;
  macosShuangpinKeymap?: boolean;
  onShuangpinProfileChange: (profile: ShuangpinProfile) => void;
  onMacosShuangpinKeymapChange?: (enabled: boolean) => void;
  /** The document's Vietnamese options; absent means the defaults, Telex with modern tone placement. */
  vietnamese?: VietnamesePreferences;
  onVietnameseChange?: (vietnamese: VietnamesePreferences) => void;
}

const cantoneseDescription = "按不带声调的粤拼输入，用 ' 分隔音节，数字键选词，候选为繁体字词";
const zhuyinDescription =
  "按大千键位输入注音符号，空格为一声，6 3 4 7 为二、三、四声和轻声，候选为繁体字词";
const vietnameseDescription = "Telex 用字母、VNI 用数字标注声调和变音，Esc 恢复原始按键";
const toneStyleDescription = "新式把声调标在主元音上（hoà），旧式按传统位置标注（hòa）";

/** Shared scheme-specific controls for desktop input settings. */
export function InputSchemeDetailsSection({
  scheme,
  shuangpinProfile,
  macos,
  hasTouchKeyboardSchemes,
  macosShuangpinKeymap,
  onShuangpinProfileChange,
  onMacosShuangpinKeymapChange,
  vietnamese,
  onVietnameseChange,
}: InputSchemeDetailsSectionProps) {
  // 双拼方案 and 五笔方案 only concern the pinyin and Wubi schemes; Cantonese and Zhuyin have rows of their own.
  const hideChineseSchemeOptions =
    hasTouchKeyboardSchemes ||
    scheme === "japanese" ||
    scheme === "korean" ||
    scheme === "cantonese" ||
    scheme === "zhuyin" ||
    scheme === "vietnamese";
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
        <ShuangpinProfileOptions />
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
      {/* 五笔、日语、韩语各只有一个方案，选择器改不了任何东西，只在对应方案下作为说明出现。 */}
      <SelectRow
        title="五笔方案"
        hidden={hasTouchKeyboardSchemes || scheme !== "wubi"}
        value="wubi86"
        onChange={() => {}}
      >
        <WubiSchemeOption />
      </SelectRow>
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
    </>
  );
}
