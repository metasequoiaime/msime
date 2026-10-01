import android.view.KeyEvent;
import app.msime.android.VietnameseInputPolicy;
import app.msime.android.ZhuyinInputPolicy;

public final class ZhuyinInputPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ZhuyinInputPolicy.ZHUYIN_SCHEME == 6, "the shared Engine ordinal for Zhuyin is 6");
        check(ZhuyinInputPolicy.OPEN_CANDIDATE_LIST_COMMAND == 16, "MSIME_OPEN_CANDIDATE_LIST is command 16");
        check(ZhuyinInputPolicy.active(6, false) && !ZhuyinInputPolicy.active(6, true)
            && !ZhuyinInputPolicy.active(4, false) && !ZhuyinInputPolicy.active(0, false),
            "only the Zhuyin scheme outside dedicated English takes the Dachen keys");

        // The Engine lists no candidates until the list opens, so candidates on the view mean it is open.
        check(ZhuyinInputPolicy.listOpen(true, "none", 3), "candidates on a Zhuyin view mean the list is open");
        check(!ZhuyinInputPolicy.listOpen(true, "none", 0) && !ZhuyinInputPolicy.listOpen(false, "none", 3)
            && !ZhuyinInputPolicy.listOpen(true, "emoji", 3), "no list without candidates or outside Zhuyin");
        check(ZhuyinInputPolicy.opensList(true, "none", "ㄓ") && !ZhuyinInputPolicy.opensList(true, "none", "")
            && !ZhuyinInputPolicy.opensList(true, "none", null) && !ZhuyinInputPolicy.opensList(false, "none", "a"),
            "the list opens while something composes");

        check(ZhuyinInputPolicy.listDownKey(KeyEvent.KEYCODE_DPAD_DOWN, false, true, false),
            "Down opens a closed list");
        check(!ZhuyinInputPolicy.listDownKey(KeyEvent.KEYCODE_DPAD_DOWN, false, true, true),
            "Down moves the highlight once the list is open");
        check(!ZhuyinInputPolicy.listDownKey(KeyEvent.KEYCODE_DPAD_DOWN, true, true, false)
            && !ZhuyinInputPolicy.listDownKey(KeyEvent.KEYCODE_DPAD_DOWN, false, false, false)
            && !ZhuyinInputPolicy.listDownKey(KeyEvent.KEYCODE_DPAD_UP, false, true, false),
            "a modified Down, an empty composition or another arrow is not the list key");

        String composing = "1234567890,./;- ";
        check(ZhuyinInputPolicy.engineKey(true, '1', composing) && ZhuyinInputPolicy.engineKey(true, ',', composing)
            && ZhuyinInputPolicy.engineKey(true, ' ', composing),
            "a spelling symbol is the Engine's before it is a candidate digit or a paging key");
        check(ZhuyinInputPolicy.engineKey(true, '<', "") && ZhuyinInputPolicy.engineKey(true, '}', null),
            "the Shift overlay is the Engine's in every state");
        check(!ZhuyinInputPolicy.engineKey(true, '3', "125890,./;-"),
            "a key the idle editor does not claim stays with the host");
        check(!ZhuyinInputPolicy.engineKey(false, '1', composing) && !ZhuyinInputPolicy.engineKey(true, 0x3105, composing)
            && !ZhuyinInputPolicy.engineKey(true, '\n', composing), "outside Zhuyin or outside ASCII the host keeps the key");

        check(ZhuyinInputPolicy.spaceIsEngineKey(true, composing),
            "touch Space on a composing conversion with its list closed is tone 1 for the Engine, not the commit command");
        check(!ZhuyinInputPolicy.spaceIsEngineKey(true, "125890,./;-") && !ZhuyinInputPolicy.spaceIsEngineKey(true, "0,./;-"),
            "idle or with the list open, touch Space keeps the plain space and the first-row pick");
        check(!ZhuyinInputPolicy.spaceIsEngineKey(false, composing), "outside Zhuyin touch Space is unchanged");

        check(VietnameseInputPolicy.VIETNAMESE_SCHEME == 7, "the shared Engine ordinal for Vietnamese is 7");
        check(VietnameseInputPolicy.active(7, false) && !VietnameseInputPolicy.active(7, true)
            && !VietnameseInputPolicy.active(4, false), "only the Vietnamese scheme outside dedicated English composes Vietnamese");
        System.out.println("Android Zhuyin and Vietnamese input policy: list, Down key, engine keys and activity passed");
    }
}
