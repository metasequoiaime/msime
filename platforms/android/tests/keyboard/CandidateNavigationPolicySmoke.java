import android.view.KeyEvent;
import app.msime.android.CandidateNavigationPolicy;
import app.msime.android.CandidateNavigationPolicy.Bindings;

/** Which hardware keys page the candidate list, and what an unticked pair must not do. */
public final class CandidateNavigationPolicySmoke {
    static final int NEXT_PAGE = 100;
    static final int PREVIOUS_PAGE = 101;
    static final int NEXT_CANDIDATE = 102;
    static final int PREVIOUS_CANDIDATE = 103;
    static final int FIRST_CANDIDATE = 104;
    static final int LAST_CANDIDATE = 105;

    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static int command(int keyCode, Bindings bindings) {
        return CandidateNavigationPolicy.commandFor(keyCode, false, bindings, false);
    }

    public static void main(String[] args) {
        Bindings all = new Bindings(true, true, true, true, true, true);

        check(command(KeyEvent.KEYCODE_MINUS, all) == PREVIOUS_PAGE
                && command(KeyEvent.KEYCODE_EQUALS, all) == NEXT_PAGE,
            "- and = page back and forward");
        check(command(KeyEvent.KEYCODE_COMMA, all) == PREVIOUS_PAGE
                && command(KeyEvent.KEYCODE_PERIOD, all) == NEXT_PAGE,
            ", and . page back and forward");
        check(command(KeyEvent.KEYCODE_LEFT_BRACKET, all) == PREVIOUS_PAGE
                && command(KeyEvent.KEYCODE_RIGHT_BRACKET, all) == NEXT_PAGE,
            "[ and ] page back and forward when bound to paging");
        check(command(KeyEvent.KEYCODE_PAGE_UP, all) == PREVIOUS_PAGE
                && command(KeyEvent.KEYCODE_PAGE_DOWN, all) == NEXT_PAGE,
            "Page Up and Page Down page");
        // Tab is the one pair whose second half is the shifted face of the same key.
        check(command(KeyEvent.KEYCODE_TAB, all) == NEXT_PAGE,
            "Tab pages forward");
        check(CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_TAB, true, all, false) == PREVIOUS_PAGE,
            "Shift+Tab pages back");
        // Arrows move the highlight rather than the page, which is a different pair of commands.
        check(command(KeyEvent.KEYCODE_DPAD_UP, all) == PREVIOUS_CANDIDATE
                && command(KeyEvent.KEYCODE_DPAD_DOWN, all) == NEXT_CANDIDATE,
            "up and down move the highlighted candidate");

        // An unticked pair yields no command, so the key goes on to do what it otherwise would.
        // Swallowing it here is what turns an unticked checkbox into a dead key.
        Bindings none = new Bindings(false, false, false, false, false, false);
        for (int keyCode : new int[] {
            KeyEvent.KEYCODE_MINUS, KeyEvent.KEYCODE_EQUALS,
            KeyEvent.KEYCODE_COMMA, KeyEvent.KEYCODE_PERIOD,
            KeyEvent.KEYCODE_LEFT_BRACKET, KeyEvent.KEYCODE_RIGHT_BRACKET,
            KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_PAGE_DOWN,
            KeyEvent.KEYCODE_TAB, KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN,
        }) {
            check(command(keyCode, none) == CandidateNavigationPolicy.NONE,
                "an unticked pair claims nothing, key code " + keyCode);
        }
        check(CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_TAB, true, none, false)
                == CandidateNavigationPolicy.NONE,
            "Shift+Tab is unclaimed when Tab paging is off");

        // Each switch governs only its own pair.
        Bindings onlyComma = new Bindings(false, true, false, false, false, false);
        check(command(KeyEvent.KEYCODE_COMMA, onlyComma) == PREVIOUS_PAGE,
            "the comma pair is on");
        check(command(KeyEvent.KEYCODE_MINUS, onlyComma) == CandidateNavigationPolicy.NONE,
            "the minus pair stays off while the comma pair is on");

        // The punctuation pairs carry a different mark on their shifted face; that mark is the
        // user's to type while composing, so it is never a paging key.
        for (int keyCode : new int[] {
            KeyEvent.KEYCODE_MINUS, KeyEvent.KEYCODE_EQUALS,
            KeyEvent.KEYCODE_COMMA, KeyEvent.KEYCODE_PERIOD,
            KeyEvent.KEYCODE_LEFT_BRACKET, KeyEvent.KEYCODE_RIGHT_BRACKET,
        }) {
            check(CandidateNavigationPolicy.commandFor(keyCode, true, all, false)
                    == CandidateNavigationPolicy.NONE,
                "the shifted face is not a paging key, key code " + keyCode);
        }

        check(command(KeyEvent.KEYCODE_A, all) == CandidateNavigationPolicy.NONE,
            "an unrelated key is never claimed");
        check(command(KeyEvent.KEYCODE_MINUS, null) == CandidateNavigationPolicy.NONE,
            "absent bindings claim nothing rather than throwing");

        // Japanese romaji needs - for the long-vowel mark, so that pair never pages there however
        // the preference is set. The other pairs are unaffected: they carry no kana input.
        check(CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_MINUS, false, all, true)
                == CandidateNavigationPolicy.NONE
                && CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_EQUALS, false, all, true)
                == CandidateNavigationPolicy.NONE,
            "- and = never page in the Japanese scheme");
        check(CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_COMMA, false, all, true)
                == PREVIOUS_PAGE
                && CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_PAGE_DOWN, false, all, true)
                == NEXT_PAGE,
            "the other pairs still page in the Japanese scheme");

        // Home and End reach the ends of the whole list. No binding gates them - the source does not
        // offer them as a rebindable pair - so they answer even when every switch is off, which is
        // what these two assertions pin. The caller only consults this policy while something is
        // being composed, so with an empty composition they remain the editor's caret keys.
        check(command(KeyEvent.KEYCODE_MOVE_HOME, all) == FIRST_CANDIDATE
                && command(KeyEvent.KEYCODE_MOVE_END, all) == LAST_CANDIDATE,
            "Home and End reach the first and last candidate");
        check(command(KeyEvent.KEYCODE_MOVE_HOME, none) == FIRST_CANDIDATE
                && command(KeyEvent.KEYCODE_MOVE_END, none) == LAST_CANDIDATE,
            "Home and End are not one of the rebindable pairs");
        // The Japanese exception is about the long-vowel mark on - and =, and does not reach these.
        check(CandidateNavigationPolicy.commandFor(KeyEvent.KEYCODE_MOVE_END, false, all, true)
                == LAST_CANDIDATE,
            "Home and End are unaffected by the Japanese scheme");

        // The shared defaults: brackets off because 以词定字 owns that pair by default.
        Bindings defaults = Bindings.defaults();
        check(defaults.minusEqual() && defaults.commaPeriod() && defaults.tab()
                && defaults.pageUpDown() && defaults.arrows() && !defaults.brackets(),
            "the defaults match crates/client-core NavigationPreferences::default");
        System.out.println("Android candidate navigation policy passed");
    }
}
