package app.msime.android;

import android.view.KeyEvent;

/**
 * How this host drives the Korean scheme, whose Engine is a Dubeolsik Hangul syllable automaton.
 *
 * <p>The automaton belongs to the Engine. This host sends the QWERTY letters with their case, marks the composing Hangul inline, and inserts whatever a transition commits before it lets a declined key do its own work. It keeps no jamo composition table of its own.
 *
 * <p>The only candidates the scheme ever has are the Hanja of the composing syllable, listed after {@link #CONVERT_HANJA_COMMAND} and gone again when the list closes. The table and the list's rules are the Engine's too (the Korean contract in msime_client.h); this host only decides which keys reach the list while it is open.
 */
public final class KoreanInputPolicy {
    /** `SchemeType::Korean`: the value `View.scheme` and `commit_context.scheme` carry for this scheme. */
    public static final int KOREAN_SCHEME = 4;
    /** Shared host command 16, `MSIME_CONVERT_HANJA`: list the Hanja of the composing syllable, or close the list when it is open. */
    public static final int CONVERT_HANJA_COMMAND = 16;
    private static final int NEXT_CANDIDATE = 102;
    private static final int PREVIOUS_CANDIDATE = 103;
    /** No command for this key. */
    public static final int NONE = -1;

    private KoreanInputPolicy() {}

    /** Whether the Engine is composing Hangul: the Korean scheme outside dedicated English. Korean has no local modes. */
    public static boolean active(int scheme, boolean dedicatedEnglish) {
        return scheme == KOREAN_SCHEME && !dedicatedEnglish;
    }

    /**
     * The text to mark inline in the editor.
     *
     * <p>For Korean that is the composing Hangul (`View.reading`, equal to `View.preedit`): `editing_text` only holds the key letters of the open syllable, so drawing it would put `gks` in the document where 한 belongs. `editing_text` still decides whether anything is composing, because it is non-empty exactly while a syllable is open. Korean never holds a phrase prefix. Every other scheme keeps the shared rule.
     */
    public static String composing(boolean korean, String phrasePrefix, String editingText,
                                   String reading) {
        if (!korean) return PhrasePreeditPolicy.composing(phrasePrefix, editingText);
        if (editingText == null || editingText.isEmpty() || reading == null) return "";
        return reading;
    }

    /**
     * The character a hardware key sends in Korean.
     *
     * <p>Shift decides the case and Caps Lock does not: the case of a Korean letter is not capitalisation but a different jamo (Shift+R is ㄲ), so a latched Caps Lock must not turn every ㄱ into ㄲ. Anything that is not an ASCII letter is passed through unchanged.
     */
    public static int hardwareCharacter(int unicode, boolean shift) {
        boolean letter = (unicode >= 'a' && unicode <= 'z') || (unicode >= 'A' && unicode <= 'Z');
        if (!letter) return unicode;
        return shift ? Character.toUpperCase(unicode) : Character.toLowerCase(unicode);
    }

    /**
     * Whether the Hanja list of the composing syllable is open.
     *
     * <p>That is the Korean rules with candidates on the view: the Engine offers none in this scheme until the Hanja command, so no separate view field is needed. A local mode or dedicated English keeps its own rules inside the scheme and never has a Hanja list.
     */
    public static boolean hanjaListOpen(boolean korean, String localMode, int candidateCount) {
        return korean && "none".equals(localMode) && candidateCount > 0;
    }

    /**
     * How many cancel commands it takes to drop the composition without writing it.
     *
     * <p>With the Hanja list open the first cancel only closes the list and leaves the syllable composing (the Korean contract in msime_client.h), so a host action that discards the composition, such as a held delete or the paging row's cancel key, sends a second one. Otherwise one cancel ends any composition.
     */
    public static int cancelsToDiscard(boolean hanjaListOpen) {
        return hanjaListOpen ? 2 : 1;
    }

    /**
     * Whether the Hanja command applies, which is also when the candidate bar shows its 漢 button: while a syllable composes, and so also while its list is open, where the command closes it.
     *
     * <p>A lone jamo composes too and has no Hanja. The Engine answers the command unhandled then and nothing changes; this host does not tell the two apart, because that would take a jamo table of its own.
     */
    public static boolean convertsHanja(boolean korean, String localMode, String editingText) {
        return korean && "none".equals(localMode) && editingText != null && !editingText.isEmpty();
    }

    /**
     * Whether a hardware key is the Hanja key: F9 with no modifier, the key ibus-hangul and fcitx5-hangul convert with and the one the Linux hosts use. Android has no key code for a Korean keyboard's own Hanja key; a lone right Ctrl tap, which is where many keyboards put it, is decided on its release by the service.
     */
    public static boolean hanjaKey(int keyCode, boolean shift, boolean ctrl, boolean alt, boolean meta) {
        return keyCode == KeyEvent.KEYCODE_F9 && !shift && !ctrl && !alt && !meta;
    }

    /**
     * Whether a key that pages or takes a character from a word elsewhere stays punctuation while the Hanja list is open: `-` `=` `[` `]` `,` `.`.
     *
     * <p>The Engine closes the list and writes the Hangul with the mark, as every other host does with the list open, so a mark typed after a syllable never turns a page or picks a Hanja instead.
     */
    public static boolean hanjaListMark(int keyCode) {
        return switch (keyCode) {
            case KeyEvent.KEYCODE_MINUS, KeyEvent.KEYCODE_EQUALS,
                 KeyEvent.KEYCODE_LEFT_BRACKET, KeyEvent.KEYCODE_RIGHT_BRACKET,
                 KeyEvent.KEYCODE_COMMA, KeyEvent.KEYCODE_PERIOD -> true;
            default -> false;
        };
    }

    /**
     * The command Left or Right sends while the Hanja list is open, or {@link #NONE}.
     *
     * <p>A syllable has no caret inside it, so the caret commands would only write the Hangul out and hand the key to the editor. Across the strip they move the highlight instead, as the arrows do on Windows and in a horizontal list on macOS. They follow the arrow switch of the shared navigation preference: with it off the key keeps its caret meaning.
     */
    public static int hanjaListArrowCommand(int keyCode, boolean arrows) {
        if (!arrows) return NONE;
        return switch (keyCode) {
            case KeyEvent.KEYCODE_DPAD_LEFT -> PREVIOUS_CANDIDATE;
            case KeyEvent.KEYCODE_DPAD_RIGHT -> NEXT_CANDIDATE;
            default -> NONE;
        };
    }
}
