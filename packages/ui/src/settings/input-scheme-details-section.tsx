import { Row, Segmented, Select, Switch } from "../core/platform-controls";
import { japaneseInputSchemeOptions } from "./input-scheme-options";
import { SettingToggle } from "./setting-toggle";

export type InputSchemeDetailsScheme = "quanpin" | "shuangpin" | "wubi" | "japanese";
export type ShuangpinProfile = "xiaohe" | "ziranma" | "shoudao" | "microsoft";

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
}

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
}: InputSchemeDetailsSectionProps) {
  const hideChineseSchemeOptions = hasTouchKeyboardSchemes || scheme === "japanese";

  if (grouped) {
    return (
      <>
        <Row title="双拼方案" hidden={hideChineseSchemeOptions}>
          <Select
            disabled={macos && scheme !== "shuangpin"}
            value={shuangpinProfile}
            onChange={(event) => onShuangpinProfileChange(event.target.value as ShuangpinProfile)}
          >
            <option value="xiaohe">小鹤双拼</option>
            <option value="ziranma">自然码双拼</option>
            <option value="shoudao">首道双拼</option>
            <option value="microsoft">微软双拼</option>
          </Select>
        </Row>
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
        <Row title="五笔方案" hidden={hideChineseSchemeOptions}>
          <Select value="wubi86" onChange={() => {}}>
            <option value="wubi86">86 五笔</option>
          </Select>
        </Row>
        <Row
          title="日语方案"
          description="直接输入罗马音，提供平假名、片假名及日语词库候选"
          hidden={hasTouchKeyboardSchemes || scheme !== "japanese"}
        >
          <Segmented options={japaneseInputSchemeOptions} value="romaji" onChange={() => {}} />
        </Row>
      </>
    );
  }

  return (
    <>
      <div className="section" hidden={hideChineseSchemeOptions}>
        <label className="section-header">
          <span className="section-title">双拼方案</span>
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
            <option value="xiaohe">小鹤双拼</option>
            <option value="ziranma">自然码双拼</option>
            <option value="shoudao">首道双拼</option>
            <option value="microsoft">微软双拼</option>
          </select>
        </label>
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
        <label className="section-header">
          <span className="section-title">五笔方案</span>
          <select aria-label="五笔方案" value="wubi86" onChange={() => {}}>
            <option value="wubi86">86 五笔</option>
          </select>
        </label>
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
    </>
  );
}
