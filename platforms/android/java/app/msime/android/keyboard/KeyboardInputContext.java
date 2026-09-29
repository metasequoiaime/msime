package app.msime.android;

/** Keeps a field-requested English mode temporary and preserves manual choices within a field. */
public final class KeyboardInputContext {
    private long documentIdentifier = Long.MIN_VALUE;
    private boolean prefersLatin;
    private Boolean englishBeforeLatinField;

    public Boolean englishOverride(boolean nextPrefersLatin, long nextDocumentIdentifier,
                                   boolean isEnglish) {
        if (documentIdentifier == nextDocumentIdentifier && prefersLatin == nextPrefersLatin) {
            return null;
        }
        documentIdentifier = nextDocumentIdentifier;
        prefersLatin = nextPrefersLatin;
        if (nextPrefersLatin) {
            if (englishBeforeLatinField == null) englishBeforeLatinField = isEnglish;
            return true;
        }
        Boolean restored = englishBeforeLatinField;
        englishBeforeLatinField = null;
        return restored;
    }
}
