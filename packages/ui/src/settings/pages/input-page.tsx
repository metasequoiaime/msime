import {
  touchKeyboardSchemeOptions,
  selectTouchKeyboardScheme,
} from "../touch-keyboard-scheme-helpers";
import { defaultNavigation } from "../navigation-section";
import type { Preferences, TouchKeyboardScheme } from "../../index";
import { useSettingsForm } from "../settings-form-context";
import * as settings from "../settings-style";
import { GroupList, Row, Segmented, Select, Switch } from "../../core/platform-controls";
import { InputModeSection } from "../input-mode-section";
import { TouchKeyboardSchemesSection } from "../touch-keyboard-schemes-section";
import { WubiSection } from "../wubi-section";
import { WordCharacterSection } from "../word-character-section";
import { LearningSection } from "../learning-section";
import { DefaultImeModeSection } from "../default-ime-mode-section";
import { ImeModeScopeSection } from "../ime-mode-scope-section";
import { TraditionalChineseOutputSection } from "../traditional-chinese-output-section";
import { CloudCandidatesSection } from "../cloud-candidates-section";
import { FrequencySection } from "../frequency-section";
import { LocalModesSection } from "../local-modes-section";
import { CharacterWidthRow } from "../punctuation-section";
import { InputModeHudSection } from "../input-mode-hud-section";
import { createUtilitiesSettingsActions } from "../utilities-settings-actions";
import { createSettingsDraftActions } from "../settings-draft-actions";
import {
  chineseInputSchemeOptions,
  japaneseInputSchemeOptions,
} from "../input-scheme-options";


/** The 输入 page of the settings form. */
export function InputSettingsPage() {
  const {
    client,
    iosPlatform,
    mobilePlatform,
    macosPlatform,
    showModeScope,
    showCharacterWidth,
    showInputModeHUD,
    draft,
    setDraft,
    busy,
    page,
    macosShuangpinKeymap,
    setShuangpinKeymap,
    macosWubiAutoCommitUnique,
    setWubiAutoCommitUnique,
    wordCharacter,
    frequency,
    touchKeyboardSchemes,
    selectedTouchKeyboardScheme,
    setTouchKeyboardSchemeEnabled,
    localModes,
  } = useSettingsForm();
  const { onLocalModesChange } = createUtilitiesSettingsActions({ draft, setDraft });
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const chineseSchemes = draft.scheme !== "japanese";
  const navigation = draft.navigation ?? defaultNavigation;
  return (
    <fieldset disabled={busy} hidden={page !== "input"} aria-label="输入">
      {/* The groups keep the reference window's order of these settings; each row is present whenever the reference shows it and hidden, not removed, while the chosen scheme makes it moot, as the reference does. */}
      <div className={settings.groups}>
        <GroupList title="方案">
          <InputModeSection
            scheme={draft.scheme}
            lastChineseScheme={draft.last_chinese_scheme}
            hidden={client.touchKeyboardSchemes}
            onChange={onPreferencesChange}
          />
          {client.touchKeyboardSchemes && (
            <TouchKeyboardSchemesSection
              options={touchKeyboardSchemeOptions}
              enabled={touchKeyboardSchemes.enabled}
              selected={selectedTouchKeyboardScheme}
              onSelect={(scheme) =>
                setDraft(selectTouchKeyboardScheme(draft, scheme as TouchKeyboardScheme))
              }
              onToggle={(scheme, enabled) =>
                setTouchKeyboardSchemeEnabled(scheme as TouchKeyboardScheme, enabled)
              }
            />
          )}
          <Row title="输入方案" hidden={client.touchKeyboardSchemes || !chineseSchemes}>
            <Segmented
              options={chineseInputSchemeOptions}
              value={
                draft.scheme === "shuangpin" || draft.scheme === "wubi" ? draft.scheme : "quanpin"
              }
              onChange={(scheme) =>
                onPreferencesChange({ scheme, last_chinese_scheme: scheme })
              }
            />
          </Row>
          <Row title="双拼方案" hidden={client.touchKeyboardSchemes || !chineseSchemes}>
            {/* The source disables this menu unless Shuangpin is the active scheme (`_shuangpinSchemeButton.enabled = storedScheme == 1`): until then the choice changes nothing, and a live control that does nothing reads as a setting being ignored. Other hosts keep it always editable. */}
            <Select
              disabled={macosPlatform && draft.scheme !== "shuangpin"}
              value={draft.shuangpin_profile}
              onChange={(event) =>
                onPreferencesChange({
                  shuangpin_profile: event.target.value as Preferences["shuangpin_profile"],
                })
              }
            >
              <option value="xiaohe">小鹤双拼</option>
              <option value="ziranma">自然码双拼</option>
              <option value="shoudao">首道双拼</option>
              <option value="microsoft">微软双拼</option>
            </Select>
          </Row>
          {macosPlatform &&
            client.loadMacosShuangpinKeymap &&
            macosShuangpinKeymap !== undefined && (
              <Row
                title="输入时显示双拼键位提示"
                description="双拼输入时显示当前方案的键位图，完成上屏后自动隐藏。"
                hidden={client.touchKeyboardSchemes || draft.scheme !== "shuangpin"}
              >
                <Switch checked={macosShuangpinKeymap} onChange={setShuangpinKeymap} />
              </Row>
            )}
          <Row title="五笔方案" hidden={client.touchKeyboardSchemes || !chineseSchemes}>
            <Select value="wubi86" onChange={() => {}}>
              <option value="wubi86">86 五笔</option>
            </Select>
          </Row>
          {((client.touchKeyboardSchemes && touchKeyboardSchemes.enabled.includes("wubi")) ||
            draft.scheme === "wubi") && (
            <WubiSection
              preferences={draft}
              autoCommitUnique={macosPlatform ? macosWubiAutoCommitUnique : undefined}
              onChange={onPreferencesChange}
              onAutoCommitUniqueChange={setWubiAutoCommitUnique}
            />
          )}
          <Row
            title="日语方案"
            description="直接输入罗马音，提供平假名、片假名及日语词库候选"
            hidden={client.touchKeyboardSchemes || chineseSchemes}
          >
            <Segmented options={japaneseInputSchemeOptions} value="romaji" onChange={() => {}} />
          </Row>
        </GroupList>
        <GroupList title="选词">
          <WordCharacterSection
            preferences={wordCharacter}
            navigation={navigation}
            ios={iosPlatform}
            onChange={(next) =>
              onPreferencesChange({
                word_character: next.wordCharacter,
                navigation: next.navigation,
              })
            }
          />
          <LearningSection
            value={draft.learning}
            onChange={(learning) => onPreferencesChange({ learning })}
          />
        </GroupList>
        <GroupList title="中英文">
          <DefaultImeModeSection
            value={draft.default_ime_mode}
            onChange={(default_ime_mode) => onPreferencesChange({ default_ime_mode })}
          />
          {showModeScope && (
            <ImeModeScopeSection
              value={draft.ime_mode_scope}
              onChange={(ime_mode_scope) => onPreferencesChange({ ime_mode_scope })}
            />
          )}
          {/* macOS keeps this with the chords that trigger it, on the shortcut page. */}
          {showInputModeHUD && !macosPlatform && (
            <InputModeHudSection
              value={draft.input_mode_hud}
              onChange={(input_mode_hud) => onPreferencesChange({ input_mode_hud })}
            />
          )}
        </GroupList>
        <GroupList title="输出">
          {showCharacterWidth && (
            <CharacterWidthRow
              preferences={draft}
              onChange={onPreferencesChange}
            />
          )}
          <TraditionalChineseOutputSection
            value={draft.traditional_chinese_output}
            onChange={(traditional_chinese_output) =>
              onPreferencesChange({ traditional_chinese_output })
            }
          />
          <CloudCandidatesSection
            value={draft.cloud_candidates}
            onChange={(cloud_candidates) => onPreferencesChange({ cloud_candidates })}
          />
        </GroupList>
        <GroupList title="整句联想">
          <Row
            title="本地整句联想"
            description="把词库组合出的整句加入候选；关闭后仍保留单词候选。"
          >
            <Switch
              checked={draft.sentence_association?.word_lattice ?? true}
              onChange={(checked) =>
                onPreferencesChange({
                  sentence_association: {
                    ...draft.sentence_association,
                    word_lattice: checked,
                  },
                })
              }
            />
          </Row>
          <Row
            title={mobilePlatform ? "键盘神经联想" : "桌面神经联想"}
            description="使用随包的神经模型重排整句候选；没有模型时保持现有候选。"
          >
            <Switch
              checked={
                mobilePlatform
                  ? (draft.sentence_association?.neural_keyboard ?? false)
                  : (draft.sentence_association?.neural_desktop ?? false)
              }
              onChange={(checked) =>
                onPreferencesChange({
                  sentence_association: {
                    ...draft.sentence_association,
                    ...(mobilePlatform
                      ? { neural_keyboard: checked }
                      : { neural_desktop: checked }),
                  },
                })
              }
            />
          </Row>
        </GroupList>
        <FrequencySection
          preferences={frequency}
          onChange={(frequency) => onPreferencesChange({ frequency })}
        />
        <LocalModesSection
          preferences={localModes}
          ios={iosPlatform}
          onChange={onLocalModesChange}
        />
      </div>
    </fieldset>
  );
}
