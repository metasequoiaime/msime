import { Row, Segmented, Switch } from "../core/platform-controls";
import type { InputScheme, VietnamesePreferences } from "../index";
import {
  cantoneseInputSchemeOptions,
  japaneseInputSchemeOptions,
  koreanInputSchemeOptions,
  vietnameseInputMethodOptions,
  vietnameseToneStyleOptions,
  zhuyinLayoutOptions,
} from "./input-scheme-options";
import { SettingToggle } from "./setting-toggle";
import { SettingField } from "./setting-field";
import { SelectRow } from "./select-row";

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
  /** Uses the shared settings-page primitives instead of the legacy panel markup. */
  grouped?: boolean;
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
  grouped = false,
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

  if (grouped) {
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
          <Row
            title="输入时显示双拼键位提示"
            description="双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。"
            hidden={hasTouchKeyboardSchemes || scheme !== "shuangpin"}
          >
            <Switch
              checked={macosShuangpinKeymap}
              onChange={onMacosShuangpinKeymapChange ?? (() => {})}
            />
          </Row>
        )}
        <SelectRow
          title="五笔方案"
          hidden={hideChineseSchemeOptions}
          value="wubi86"
          onChange={() => {}}
        >
          <WubiSchemeOption />
        </SelectRow>
        <Row
          title="日语方案"
          description="直接输入罗马音，提供平假名、片假名及日语词库候选"
          hidden={hasTouchKeyboardSchemes || scheme !== "japanese"}
        >
          <Segmented options={japaneseInputSchemeOptions} value="romaji" onChange={() => {}} />
        </Row>
        <Row
          title="韩语方案"
          description="按两套式（두벌식）键位输入韩文字母，自动拼成音节，标点为半角"
          hidden={hasTouchKeyboardSchemes || scheme !== "korean"}
        >
          <Segmented options={koreanInputSchemeOptions} value="dubeolsik" onChange={() => {}} />
        </Row>
        <Row
          title="粤拼方案"
          description={cantoneseDescription}
          hidden={hasTouchKeyboardSchemes || scheme !== "cantonese"}
        >
          <Segmented options={cantoneseInputSchemeOptions} value="jyutping" onChange={() => {}} />
        </Row>
        <Row
          title="注音键盘"
          description={zhuyinDescription}
          hidden={hasTouchKeyboardSchemes || scheme !== "zhuyin"}
        >
          <Segmented options={zhuyinLayoutOptions} value="dachen" onChange={() => {}} />
        </Row>
        <Row
          title="越南语方案"
          description={vietnameseDescription}
          hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
        >
          <Segmented
            options={vietnameseInputMethodOptions}
            value={inputMethod}
            onChange={(input_method) => changeVietnamese({ input_method })}
          />
        </Row>
        <Row
          title="声调位置"
          description={toneStyleDescription}
          hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
        >
          <Segmented
            options={vietnameseToneStyleOptions}
            value={toneStyle}
            onChange={(tone_style) => changeVietnamese({ tone_style })}
          />
        </Row>
      </>
    );
  }

  return (
    <>
      <div className="section" hidden={hideChineseSchemeOptions}>
        <SettingField label="双拼方案">
          {/* The source disables this menu unless Shuangpin is the active scheme
              (`_shuangpinSchemeButton.enabled = storedScheme == 1`): until then the
              choice changes nothing, and a live control that does nothing reads as a
              setting being ignored. Other hosts keep it always editable. */}
          <select
            aria-label="双拼方案"
            disabled={macos && scheme !== "shuangpin"}
            value={shuangpinProfile}
            onChange={(event) => onShuangpinProfileChange(event.target.value as ShuangpinProfile)}
          >
            <ShuangpinProfileOptions />
          </select>
        </SettingField>
      </div>
      {macosShuangpinKeymap !== undefined && (
        <div className="section" hidden={hasTouchKeyboardSchemes || scheme !== "shuangpin"}>
          <SettingToggle
            label="输入时显示双拼键位提示"
            description="双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。"
            ariaLabel="输入时显示双拼键位提示"
            checked={macosShuangpinKeymap}
            compact
            onChange={(enabled) => onMacosShuangpinKeymapChange?.(enabled)}
          />
        </div>
      )}
      <div className="section" hidden={hideChineseSchemeOptions}>
        <SettingField label="五笔方案">
          <select aria-label="五笔方案" value="wubi86" onChange={() => {}}>
            <WubiSchemeOption />
          </select>
        </SettingField>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="japanese-scheme-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "japanese"}
      >
        <div className="section-title" id="japanese-scheme-title">
          日语方案
        </div>
        <div className="input-option-content">
          <label className="radio-option">
            <input type="radio" name="japanese-scheme" checked readOnly />
            <span>罗马音</span>
          </label>
        </div>
        <div className="input-setting-description japanese-scheme-description">
          直接输入罗马音，提供平假名、片假名及日语词库候选
        </div>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="korean-scheme-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "korean"}
      >
        <div className="section-title" id="korean-scheme-title">
          韩语方案
        </div>
        <div className="input-option-content">
          <label className="radio-option">
            <input type="radio" name="korean-scheme" checked readOnly />
            <span>两套式</span>
          </label>
        </div>
        <div className="input-setting-description korean-scheme-description">
          按两套式（두벌식）键位输入韩文字母，自动拼成音节，标点为半角
        </div>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="cantonese-scheme-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "cantonese"}
      >
        <div className="section-title" id="cantonese-scheme-title">
          粤拼方案
        </div>
        <div className="input-option-content">
          <label className="radio-option">
            <input type="radio" name="cantonese-scheme" checked readOnly />
            <span>粤拼</span>
          </label>
        </div>
        <div className="input-setting-description">{cantoneseDescription}</div>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="zhuyin-layout-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "zhuyin"}
      >
        <div className="section-title" id="zhuyin-layout-title">
          注音键盘
        </div>
        <div className="input-option-content">
          <label className="radio-option">
            <input type="radio" name="zhuyin-layout" checked readOnly />
            <span>大千</span>
          </label>
        </div>
        <div className="input-setting-description">{zhuyinDescription}</div>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="vietnamese-scheme-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
      >
        <div className="section-title" id="vietnamese-scheme-title">
          越南语方案
        </div>
        <div className="input-option-content">
          {vietnameseInputMethodOptions.map(({ value, label }) => (
            <label className="radio-option" key={value}>
              <input
                type="radio"
                name="vietnamese-input-method"
                value={value}
                checked={inputMethod === value}
                onChange={() => changeVietnamese({ input_method: value })}
              />
              <span>{label}</span>
            </label>
          ))}
        </div>
        <div className="input-setting-description">{vietnameseDescription}</div>
      </div>
      <div
        className="section"
        role="group"
        aria-labelledby="vietnamese-tone-style-title"
        hidden={hasTouchKeyboardSchemes || scheme !== "vietnamese"}
      >
        <div className="section-title" id="vietnamese-tone-style-title">
          声调位置
        </div>
        <div className="input-option-content">
          {vietnameseToneStyleOptions.map(({ value, label }) => (
            <label className="radio-option" key={value}>
              <input
                type="radio"
                name="vietnamese-tone-style"
                value={value}
                checked={toneStyle === value}
                onChange={() => changeVietnamese({ tone_style: value })}
              />
              <span>{label}</span>
            </label>
          ))}
        </div>
        <div className="input-setting-description">{toneStyleDescription}</div>
      </div>
    </>
  );
}
