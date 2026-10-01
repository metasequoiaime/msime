import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { CandidateEnglishGlossSection } from "./candidate-english-gloss-section";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { MixedInputSection, type MixedInputPreferences } from "./mixed-input-section";

export interface InputLanguageOptionsSectionProps {
  grouped?: boolean;
  includeMixed?: boolean;
  mixedInput?: MixedInputPreferences;
  includeCandidateControls?: boolean;
  showCandidateEnglishGloss?: boolean;
  candidateEnglishGloss?: boolean;
  showEnglishSuggestions?: boolean;
  englishSuggestions?: boolean;
  betweenMixedAndCandidates?: ReactNode;
  /** 紧跟在「显示英文释义」之后显示，放调整这些释义的控件。 */
  afterCandidateEnglishGloss?: ReactNode;
  onMixedInputChange?: (value: MixedInputPreferences) => void;
  onCandidateEnglishGlossChange?: (value: boolean) => void;
  onEnglishSuggestionsChange?: (value: boolean) => void;
}

/** Shared controls for mixed Chinese and English input and English candidate options. */
export function InputLanguageOptionsSection({
  grouped = false,
  includeMixed = false,
  mixedInput,
  includeCandidateControls = false,
  showCandidateEnglishGloss = false,
  candidateEnglishGloss,
  showEnglishSuggestions = false,
  englishSuggestions,
  betweenMixedAndCandidates,
  afterCandidateEnglishGloss,
  onMixedInputChange,
  onCandidateEnglishGlossChange,
  onEnglishSuggestionsChange,
}: InputLanguageOptionsSectionProps) {
  const content = (
    <>
      {includeMixed && mixedInput && onMixedInputChange && (
        <MixedInputSection preferences={mixedInput} onChange={onMixedInputChange} />
      )}
      {betweenMixedAndCandidates}
      {includeCandidateControls && showCandidateEnglishGloss && onCandidateEnglishGlossChange && (
        <CandidateEnglishGlossSection
          value={candidateEnglishGloss}
          onChange={onCandidateEnglishGlossChange}
        />
      )}
      {afterCandidateEnglishGloss}
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
