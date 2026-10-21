import type { ReactNode } from "react";
import { MoreOptions } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";
import {
  translationLanguages,
  type TranslationLanguage,
  type TranslationLanguageOption,
  type TranslationSecondaryLanguage,
  type TranslationSecondaryLanguageOption,
} from "./translation-language-helpers";
export type {
  TranslationLanguage,
  TranslationLanguageOption,
  TranslationSecondaryLanguage,
  TranslationSecondaryLanguageOption,
} from "./translation-language-helpers";

export interface CandidateTranslationOptionsSectionProps {
  enabled: boolean;
  targetLanguage: TranslationLanguage;
  secondaryLanguage: TranslationSecondaryLanguage | null;
  candidateGlossLanguagesEnabled: boolean;
  visibleLanguages: readonly TranslationLanguageOption[];
  visibleSecondaryLanguages: readonly TranslationSecondaryLanguageOption[];
  showSecondaryLanguage: boolean;
  showAccountTranslation: boolean;
  accountTranslation: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onTargetLanguageChange: (language: TranslationLanguage) => void;
  onSecondaryLanguageChange: (language: TranslationSecondaryLanguage) => void;
  onAccountTranslationChange: (enabled: boolean) => void;
  /** 候选释义里显示词性标记；只在宿主消费这个偏好时传入（当前只有 macOS）。 */
  showPos?: boolean;
  /** 候选释义只显示第一条；只在宿主消费这个偏好时传入（当前只有 macOS）。 */
  singleSense?: boolean;
  onShowPosChange?: (enabled: boolean) => void;
  onSingleSenseChange?: (enabled: boolean) => void;
  /** `phone` 是 HarmonyOS 手机的「翻译」组：开关注明它显示哪种语言，目标行是普通的「翻译目标语言」选项，`beforeMore` 紧随其后，第二语言和账号相关的行收进「更多选项」折叠区。`default` 布局只把第二种语言收进「更多选项」；账号开关留在外面，因为在 Android 上它是唯一的在线翻译来源。 */
  layout?: "default" | "phone";
  /** 绘制在目标语言之后、折叠区之前的行：`phone` 布局放离线释义开关，桌面「候选释义」标签放「显示读音」。 */
  beforeMore?: ReactNode;
}

/** Candidate translation enablement and language choices shared by desktop and mobile settings. */
export function CandidateTranslationOptionsSection({
  enabled,
  targetLanguage,
  secondaryLanguage,
  candidateGlossLanguagesEnabled,
  visibleLanguages,
  visibleSecondaryLanguages,
  showSecondaryLanguage,
  showAccountTranslation,
  accountTranslation,
  onEnabledChange,
  onTargetLanguageChange,
  onSecondaryLanguageChange,
  onAccountTranslationChange,
  showPos,
  singleSense,
  onShowPosChange,
  onSingleSenseChange,
  layout = "default",
  beforeMore,
}: CandidateTranslationOptionsSectionProps) {
  const phone = layout === "phone";
  // 完整列表里的语言名称不带「已保存」标记；手机列表会给已不再提供的已保存选项加上这个标记。
  const targetName =
    translationLanguages.find(([value]) => value === targetLanguage)?.[1] ?? targetLanguage;
  const enabledRow = (
    <SwitchRow
      title="候选词翻译"
      description={
        phone ? `在候选词下方显示${targetName}释义` : "为当前候选请求翻译结果并显示在候选行"
      }
      checked={enabled}
      onChange={onEnabledChange}
    />
  );
  const targetRow = (
    <SelectRow
      title={phone ? "翻译目标语言" : "目标语言"}
      aria-label="候选词翻译目标语言"
      disabled={!candidateGlossLanguagesEnabled}
      value={targetLanguage}
      onChange={(event) => onTargetLanguageChange(event.target.value as TranslationLanguage)}
    >
      {visibleLanguages.map(([value, label]) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
  const secondaryRow = showSecondaryLanguage && (
    <SelectRow
      title="第二种语言"
      description="候选词下方可同时显示第二种释义"
      aria-label="候选词翻译第二种语言"
      disabled={!candidateGlossLanguagesEnabled}
      value={secondaryLanguage ?? ""}
      onChange={(event) =>
        onSecondaryLanguageChange(event.target.value as TranslationSecondaryLanguage)
      }
    >
      {visibleSecondaryLanguages.map(([value, label]) => (
        <option key={value || "none"} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
  const accountRow = showAccountTranslation && (
    <SwitchRow
      title="使用水杉账号翻译候选词"
      description="当前页的中文候选词会发送到 api.msime.app；匿名账号在 Linux 安装后的用户初始化中自动注册；不开启则不联网翻译"
      disabled={!enabled}
      checked={accountTranslation}
      onChange={onAccountTranslationChange}
    />
  );
  // 「显示词性」和「只显示单条释义」是释义的子行：没有任何释义来源打开时没有释义可整理，两行不出现，已存的值不动。
  const glossDisplayRows = candidateGlossLanguagesEnabled && (
    <>
      {onShowPosChange && (
        <SwitchRow
          title="显示词性"
          description="显示译文中的 n.、v. 等词性标记，关闭后仅显示释义"
          checked={showPos ?? true}
          onChange={onShowPosChange}
        />
      )}
      {onSingleSenseChange && (
        <SwitchRow
          title="只显示单条释义"
          description="每个候选只显示第一条译文释义；Ctrl+Enter 仍可选择全部释义上屏"
          checked={singleSense ?? false}
          onChange={onSingleSenseChange}
        />
      )}
    </>
  );
  if (phone)
    return (
      <>
        {enabledRow}
        {targetRow}
        {glossDisplayRows}
        {beforeMore}
        {(secondaryRow || accountRow) && (
          <MoreOptions>
            {secondaryRow}
            {accountRow}
          </MoreOptions>
        )}
      </>
    );
  return (
    <>
      {enabledRow}
      {targetRow}
      {glossDisplayRows}
      {beforeMore}
      {accountRow}
      {secondaryRow && <MoreOptions>{secondaryRow}</MoreOptions>}
    </>
  );
}
