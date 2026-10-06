import android.view.KeyEvent;
import app.msime.android.EditorBridge;
import app.msime.android.KoreanInputPolicy;
import java.util.ArrayList;
import java.util.List;

public final class KoreanInputPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(KoreanInputPolicy.KOREAN_SCHEME == 4, "the shared Engine ordinal for Korean is 4");
        check(KoreanInputPolicy.active(4, false), "Korean scheme composes Hangul");
        check(!KoreanInputPolicy.active(4, true), "dedicated English is not Korean input");
        check(!KoreanInputPolicy.active(3, false) && !KoreanInputPolicy.active(0, false),
            "other schemes are not Korean");

        // The inline composition is the Hangul, never the key letters editing_text holds.
        check("녕".equals(KoreanInputPolicy.composing(true, "", "sud", "녕")),
            "Korean marks the composing syllable");
        check("".equals(KoreanInputPolicy.composing(true, "", "", "")),
            "nothing composing marks nothing");
        check("".equals(KoreanInputPolicy.composing(true, "", "", "안")),
            "editing_text decides whether a syllable is open");
        check("".equals(KoreanInputPolicy.composing(true, "", "gk", null)),
            "a missing reading is bounded to empty text");
        check("你好nihao".equals(KoreanInputPolicy.composing(false, "你好", "nihao", "")),
            "other schemes keep the shared phrase-prefix rule");

        // Shift decides the case; Caps Lock does not.
        check(KoreanInputPolicy.hardwareCharacter('R', false) == 'r',
            "Caps Lock alone types the plain consonant");
        check(KoreanInputPolicy.hardwareCharacter('r', true) == 'R'
                && KoreanInputPolicy.hardwareCharacter('R', true) == 'R',
            "Shift types the double consonant");
        check(KoreanInputPolicy.hardwareCharacter('1', true) == '1'
                && KoreanInputPolicy.hardwareCharacter(',', false) == ','
                && KoreanInputPolicy.hardwareCharacter(0, false) == 0,
            "non-letters pass through");

        // "dkssud": the fourth key finishes 안 and opens ㄴ in the same transition. The commit has to
        // land before the new syllable is marked, or the marked region would swallow it.
        List<String> writes = new ArrayList<>();
        EditorBridge bridge = new EditorBridge();
        EditorBridge.Sink sink = sink(writes);
        check(bridge.apply(sink, null, KoreanInputPolicy.composing(true, "", "dks", "안")),
            "the open syllable is marked");
        check(bridge.apply(sink, "안", KoreanInputPolicy.composing(true, "", "s", "ㄴ")),
            "an auto-committed syllable and the next composition apply together");
        // Space: handled=false, the syllable is committed and the composition emptied; the host then inserts the space.
        check(bridge.apply(sink, "녕", KoreanInputPolicy.composing(true, "", "", "")),
            "a syllable-ending key commits and clears the mark");
        check(writes.equals(List.of("begin", "compose:안", "end",
                "begin", "commit:안", "compose:ㄴ", "end",
                "begin", "commit:녕", "end")),
            "writes are ordered commit first, then the new mark: " + writes);
        // Hanja: command 16 is MSIME_CONVERT_HANJA, and the list is open exactly while the Korean view carries candidates.
        check(KoreanInputPolicy.CONVERT_HANJA_COMMAND == 16, "the shared MSIME_CONVERT_HANJA is 16");
        check(KoreanInputPolicy.hanjaListOpen(true, "none", 9), "Korean candidates are the Hanja list");
        check(!KoreanInputPolicy.hanjaListOpen(true, "none", 0), "no candidates, no list");
        check(!KoreanInputPolicy.hanjaListOpen(false, "none", 9),
            "other schemes' and dedicated English's candidates are not a Hanja list");
        check(!KoreanInputPolicy.hanjaListOpen(true, "temporary_english", 3),
            "a local mode keeps its own candidates");
        // A held delete or the paging row's cancel key drops the syllable; with the list open the first cancel only closes it.
        check(KoreanInputPolicy.cancelsToDiscard(true) == 2,
            "discarding with the Hanja list open closes the list and then drops the syllable");
        check(KoreanInputPolicy.cancelsToDiscard(false) == 1,
            "discarding without a Hanja list takes one cancel");
        check(KoreanInputPolicy.convertsHanja(true, "none", "gks"),
            "a composing syllable offers its Hanja");
        check(KoreanInputPolicy.convertsHanja(true, "none", "r"),
            "a lone jamo is left to the Engine, which declines it");
        check(!KoreanInputPolicy.convertsHanja(true, "none", "")
                && !KoreanInputPolicy.convertsHanja(true, "none", null),
            "nothing composing offers nothing");
        check(!KoreanInputPolicy.convertsHanja(false, "none", "nihao"),
            "other schemes have no Hanja command");
        check(!KoreanInputPolicy.convertsHanja(true, "temporary_english", "gks"),
            "a local mode has no Hanja command");

        // The hardware Hanja key is bare F9; Ctrl+F9 and the other chords stay shortcuts.
        check(KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F9, false, false, false, false),
            "F9 converts");
        check(!KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F9, false, true, false, false)
                && !KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F9, true, false, false, false)
                && !KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F9, false, false, true, false)
                && !KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F9, false, false, false, true),
            "a modified F9 is not the Hanja key");
        check(!KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_F8, false, false, false, false)
                && !KoreanInputPolicy.hanjaKey(KeyEvent.KEYCODE_CTRL_RIGHT, false, false, false, false),
            "other keys are not the Hanja key");

        // With the list open the paging and word-to-character marks stay punctuation.
        for (int mark : new int[] {KeyEvent.KEYCODE_MINUS, KeyEvent.KEYCODE_EQUALS,
                KeyEvent.KEYCODE_LEFT_BRACKET, KeyEvent.KEYCODE_RIGHT_BRACKET,
                KeyEvent.KEYCODE_COMMA, KeyEvent.KEYCODE_PERIOD}) {
            check(KoreanInputPolicy.hanjaListMark(mark), "mark " + mark + " stays punctuation");
        }
        for (int key : new int[] {KeyEvent.KEYCODE_PAGE_DOWN, KeyEvent.KEYCODE_TAB,
                KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_MOVE_END, KeyEvent.KEYCODE_SLASH}) {
            check(!KoreanInputPolicy.hanjaListMark(key), "key " + key + " is not a paging mark");
        }

        // Left and Right move the highlight across the list, unless the arrow switch is off.
        check(KoreanInputPolicy.hanjaListArrowCommand(KeyEvent.KEYCODE_DPAD_LEFT, true) == 103
                && KoreanInputPolicy.hanjaListArrowCommand(KeyEvent.KEYCODE_DPAD_RIGHT, true) == 102,
            "Left and Right are the previous and next candidate commands");
        check(KoreanInputPolicy.hanjaListArrowCommand(KeyEvent.KEYCODE_DPAD_LEFT, false)
                == KoreanInputPolicy.NONE,
            "with the arrows off Left keeps its caret meaning");
        check(KoreanInputPolicy.hanjaListArrowCommand(KeyEvent.KEYCODE_DPAD_UP, true)
                == KoreanInputPolicy.NONE,
            "Up and Down belong to the shared navigation policy");
        System.out.println("Android Korean input: inline Hangul, hardware case, commit order and Hanja keys passed");
    }

    private static EditorBridge.Sink sink(List<String> writes) {
        return new EditorBridge.Sink() {
            public void begin() { writes.add("begin"); }
            public boolean commit(String text) { writes.add("commit:" + text); return true; }
            public boolean compose(String text) { writes.add("compose:" + text); return true; }
            public boolean finish() { writes.add("finish"); return true; }
            public boolean select(int start, int end) { writes.add("select:" + start + "," + end); return true; }
            public void end() { writes.add("end"); }
        };
    }
}
