import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { CandidateEnglishGlossSection } from "./candidate-english-gloss-section";
import { CandidatePronunciationSection } from "./candidate-pronunciation-section";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { MixedInputSection, type MixedInputPreferences } from "./mixed-input-section";

export interface InputLanguageOptionsSectionProps {
  grouped?: boolean;
  includeMixed?: boolean;
  mixedInput?: MixedInputPreferences;
  includeCandidateControls?: boolean;
  showCandidateEnglishGloss?: boolean;
  candidateEnglishGloss?: boolean;
  showCandidatePronunciation?: boolean;
  candidatePronunciation?: boolean;
  candidatePronunciationDisabled?: boolean;
  showEnglishSuggestions?: boolean;
  englishSuggestions?: boolean;
  betweenMixedAndCandidates?: ReactNode;
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
  includeCandidateControls = false,
  showCandidateEnglishGloss = false,
  candidateEnglishGloss,
  showCandidatePronunciation = false,
  candidatePronunciation,
  candidatePronunciationDisabled,
  showEnglishSuggestions = false,
  englishSuggestions,
  betweenMixedAndCandidates,
  onMixedInputChange,
  onCandidateEnglishGlossChange,
  onCandidatePronunciationChange,
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

  return grouped ? <GroupList title="多语言候选">{content}</GroupList> : content;
}
