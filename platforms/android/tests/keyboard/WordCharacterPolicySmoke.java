import android.view.KeyEvent;
import app.msime.android.WordCharacterPolicy;
import app.msime.android.WordCharacterPolicy.Edge;

/** Which key claims 以词定字, and the three conditions that stop it claiming one. */
public final class WordCharacterPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static Edge edge(int keyCode, String binding) {
        return WordCharacterPolicy.edgeFor(keyCode, false, binding, true);
    }

    public static void main(String[] args) {
        check(WordCharacterPolicy.binding(false, "brackets").equals(WordCharacterPolicy.DISABLED),
            "the switch being off disables the binding whatever the pair says");
        check(WordCharacterPolicy.binding(true, "minus_equal").equals(WordCharacterPolicy.MINUS_EQUAL),
            "an enabled minus_equal preference binds that pair");
        check(WordCharacterPolicy.binding(true, "brackets").equals(WordCharacterPolicy.BRACKETS),
            "an enabled brackets preference binds that pair");
        check(WordCharacterPolicy.binding(true, "nonsense").equals(WordCharacterPolicy.BRACKETS),
            "an unrecognised pair falls back to the shared default rather than disabling silently");

        check(edge(KeyEvent.KEYCODE_LEFT_BRACKET, WordCharacterPolicy.BRACKETS) == Edge.FIRST,
            "[ takes the first character");
        check(edge(KeyEvent.KEYCODE_RIGHT_BRACKET, WordCharacterPolicy.BRACKETS) == Edge.LAST,
            "] takes the last character");
        check(edge(KeyEvent.KEYCODE_MINUS, WordCharacterPolicy.MINUS_EQUAL) == Edge.FIRST,
            "- takes the first character");
        check(edge(KeyEvent.KEYCODE_EQUALS, WordCharacterPolicy.MINUS_EQUAL) == Edge.LAST,
            "= takes the last character");

        // The pair that is not bound keeps typing its own symbol, which is the whole point of
        // having the choice: a user on brackets must still be able to type - and = while composing.
        check(edge(KeyEvent.KEYCODE_MINUS, WordCharacterPolicy.BRACKETS) == Edge.NONE,
            "- is not claimed while brackets are bound");
        check(edge(KeyEvent.KEYCODE_LEFT_BRACKET, WordCharacterPolicy.MINUS_EQUAL) == Edge.NONE,
            "[ is not claimed while minus_equal is bound");
        check(edge(KeyEvent.KEYCODE_LEFT_BRACKET, WordCharacterPolicy.DISABLED) == Edge.NONE,
            "nothing is claimed while the feature is off");
        check(edge(KeyEvent.KEYCODE_LEFT_BRACKET, null) == Edge.NONE,
            "a missing binding claims nothing rather than throwing");
        check(edge(KeyEvent.KEYCODE_A, WordCharacterPolicy.BRACKETS) == Edge.NONE,
            "an unrelated key is never claimed");

        check(WordCharacterPolicy.edgeFor(KeyEvent.KEYCODE_LEFT_BRACKET, true,
                WordCharacterPolicy.BRACKETS, true) == Edge.NONE,
            "Shift types the pair's other face and must not be claimed");
        check(WordCharacterPolicy.edgeFor(KeyEvent.KEYCODE_LEFT_BRACKET, false,
                WordCharacterPolicy.BRACKETS, false) == Edge.NONE,
            "with no highlighted candidate there is nothing to take a character from");

        check(Edge.FIRST.code() == 0 && Edge.LAST.code() == 1,
            "the codes are MSIME_FIRST_HAN and MSIME_LAST_HAN from the shared header");
        System.out.println("Android word-character policy passed");
    }
}
