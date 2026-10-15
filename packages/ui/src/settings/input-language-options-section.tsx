import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { CandidateEnglishGlossSection } from "./candidate-english-gloss-section";
import { CandidatePronunciationSection } from "./candidate-pronunciation-section";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { MixedInputSection } from "./mixed-input-section";
import type { MixedInputPreferences } from "./mixed-input-preferences";

export interface InputLanguageOptionsSectionProps {
  grouped?: boolean;
  includeMixed?: boolean;
  mixedInput?: MixedInputPreferences;
  /** 本版本带临时日文，决定是否列出日文混输开关；见 `MixedInputSectionProps.temporaryJapanese`。缺省为 true。 */
  temporaryJapanese?: boolean;
  includeCandidateControls?: boolean;
  showCandidateEnglishGloss?: boolean;
  candidateEnglishGloss?: boolean;
  showCandidatePronunciation?: boolean;
  candidatePronunciation?: boolean;
  candidatePronunciationDisabled?: boolean;
  showEnglishSuggestions?: boolean;
  englishSuggestions?: boolean;
  betweenMixedAndCandidates?: ReactNode;
  /** 紧跟在「显示英文释义」之后显示，放调整这些释义的控件。 */
  afterCandidateEnglishGloss?: ReactNode;
  onMixedInputChange?: (value: MixedInputPreferences) => void;
  onCandidateEnglishGlossChange?: (value: boolean) => void;
  onCandidatePronunciationChange?: (value: boolean) => void;
  onEnglishSuggestionsChange?: (value: boolean) => void;
}

/** Shared controls for mixed Chinese and English input and English candidate options. */
export function InputLanguageOptionsSection({
  grouped = false,
  includeMixed = false,
  mixedInput,
  temporaryJapanese = true,
  includeCandidateControls = false,
  showCandidateEnglishGloss = false,
  candidateEnglishGloss,
  showCandidatePronunciation = false,
  candidatePronunciation,
  candidatePronunciationDisabled,
  showEnglishSuggestions = false,
  englishSuggestions,
  betweenMixedAndCandidates,
  afterCandidateEnglishGloss,
  onMixedInputChange,
  onCandidateEnglishGlossChange,
  onCandidatePronunciationChange,
  onEnglishSuggestionsChange,
}: InputLanguageOptionsSectionProps) {
  const content = (
    <>
      {includeMixed && mixedInput && onMixedInputChange && (
        <MixedInputSection
          preferences={mixedInput}
          temporaryJapanese={temporaryJapanese}
          onChange={onMixedInputChange}
        />
      )}
      {betweenMixedAndCandidates}
      {includeCandidateControls && showCandidateEnglishGloss && onCandidateEnglishGlossChange && (
        <CandidateEnglishGlossSection
          value={candidateEnglishGloss}
          onChange={onCandidateEnglishGlossChange}
        />
      )}
      {afterCandidateEnglishGloss}
      {includeCandidateControls && showCandidatePronunciation && onCandidatePronunciationChange && (
        <CandidatePronunciationSection
          value={candidatePronunciation}
          disabled={candidatePronunciationDisabled}
          onChange={onCandidatePronunciationChange}
        />
      )}
      {includeCandidateControls && showEnglishSuggestions && onEnglishSuggestionsChange && (
        <EnglishSuggestionsSection
          value={englishSuggestions}
          onChange={onEnglishSuggestionsChange}
        />
      )}
    </>
  );

  return grouped ? <GroupList title="多语言与释义">{content}</GroupList> : content;
}
