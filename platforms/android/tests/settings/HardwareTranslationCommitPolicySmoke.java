import android.view.KeyEvent;
import app.msime.android.HardwareTranslationCommitPolicy;

/** 实体键盘 Ctrl+Enter 上屏译文（#7079）：什么时候接这个键、按义项数怎么分，以及义项页里每个键的去向。 */
public final class HardwareTranslationCommitPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** 只按着 Ctrl、首次按下、可以插入译文。 */
    static int ctrlEnter(int senseCount) {
        return HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, false,
            0, true, senseCount);
    }

    /** 义项页里不带修饰键的首次按下，页上有三个义项。 */
    static int page(int keyCode) {
        return HardwareTranslationCommitPolicy.pageKey(keyCode, false, false, false, false, 0, 3);
    }

    public static void main(String[] args) {
        // 按义项数分：没有就吃掉，一个直接上屏，几个打开义项页。释义显示关着时调用方按 0 个传入，同样吃掉。
        check(ctrlEnter(0) == HardwareTranslationCommitPolicy.CONSUME,
            "no gloss (or glosses switched off) swallows the key");
        check(ctrlEnter(1) == HardwareTranslationCommitPolicy.COMMIT, "one sense commits directly");
        check(ctrlEnter(2) == HardwareTranslationCommitPolicy.OPEN_PAGE, "two senses open the page");
        check(ctrlEnter(9) == HardwareTranslationCommitPolicy.OPEN_PAGE, "nine senses open the page");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_NUMPAD_ENTER, false, true, false,
                false, 0, true, 1) == HardwareTranslationCommitPolicy.COMMIT,
            "the numeric keypad Enter is the same key");

        // 不在组字、没有候选或方案不支持插入译文时不接，交给原来的修饰键分支。
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, false,
                0, false, 1) == HardwareTranslationCommitPolicy.NONE,
            "an ineligible state keeps the old behaviour");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, false,
                0, false, 0) == HardwareTranslationCommitPolicy.NONE,
            "an ineligible state is not swallowed even without a gloss");

        // 只认单独的 Ctrl：加了 Shift、Alt 或 Meta 是别的组合，没按 Ctrl 的回车是普通回车。
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, true, true, false, false,
                0, true, 1) == HardwareTranslationCommitPolicy.NONE, "Ctrl+Shift+Enter is not ours");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, true, false,
                0, true, 1) == HardwareTranslationCommitPolicy.NONE, "Ctrl+Alt+Enter is not ours");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, true,
                0, true, 1) == HardwareTranslationCommitPolicy.NONE, "Ctrl+Meta+Enter is not ours");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, false, false, false,
                0, true, 1) == HardwareTranslationCommitPolicy.NONE, "a bare Enter is not ours");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_A, false, true, false, false,
                0, true, 1) == HardwareTranslationCommitPolicy.NONE, "Ctrl+A is not ours");

        // 按住不放：只吃掉，不连续上屏、不反复开页。
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, false,
                1, true, 1) == HardwareTranslationCommitPolicy.CONSUME, "a repeat does not commit again");
        check(HardwareTranslationCommitPolicy.ctrlEnter(KeyEvent.KEYCODE_ENTER, false, true, false, false,
                2, true, 3) == HardwareTranslationCommitPolicy.CONSUME, "a repeat does not reopen the page");

        // 义项页：数字 1–9（主键盘或小键盘）选第几项，超出义项数的留在页内。
        check(page(KeyEvent.KEYCODE_1) == 0, "1 picks the first sense");
        check(page(KeyEvent.KEYCODE_3) == 2, "3 picks the third sense");
        check(page(KeyEvent.KEYCODE_NUMPAD_2) == 1, "the keypad 2 picks the second sense");
        check(page(KeyEvent.KEYCODE_4) == HardwareTranslationCommitPolicy.IGNORE,
            "a digit past the last sense stays on the page");
        check(page(KeyEvent.KEYCODE_9) == HardwareTranslationCommitPolicy.IGNORE,
            "9 with three senses stays on the page");
        check(page(KeyEvent.KEYCODE_0) == HardwareTranslationCommitPolicy.PASS, "0 is not a sense");

        check(page(KeyEvent.KEYCODE_SPACE) == HardwareTranslationCommitPolicy.COMMIT_HIGHLIGHT,
            "Space commits the highlighted sense");
        check(page(KeyEvent.KEYCODE_ENTER) == HardwareTranslationCommitPolicy.COMMIT_HIGHLIGHT,
            "Enter commits the highlighted sense");
        check(page(KeyEvent.KEYCODE_NUMPAD_ENTER) == HardwareTranslationCommitPolicy.COMMIT_HIGHLIGHT,
            "the keypad Enter commits the highlighted sense");

        // 会上屏或关页的键按住时只吃掉：第一下已经处理过了。
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_1, false, false, false, false, 1, 3)
                == HardwareTranslationCommitPolicy.IGNORE, "a held digit does not commit");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_SPACE, false, false, false, false, 1, 3)
                == HardwareTranslationCommitPolicy.IGNORE, "a held Space does not commit");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_ENTER, false, false, false, false, 1, 3)
                == HardwareTranslationCommitPolicy.IGNORE, "a held Enter does not commit");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_ESCAPE, false, false, false, false, 1, 3)
                == HardwareTranslationCommitPolicy.IGNORE, "a held Escape is handled once");

        // Esc 和返回只关页，组字留着。
        check(page(KeyEvent.KEYCODE_ESCAPE) == HardwareTranslationCommitPolicy.CLOSE, "Escape closes the page");
        check(page(KeyEvent.KEYCODE_BACK) == HardwareTranslationCommitPolicy.CLOSE, "Back closes the page");

        // 方向键移动高亮，可以按住连续移动。
        check(page(KeyEvent.KEYCODE_DPAD_UP) == HardwareTranslationCommitPolicy.PREVIOUS, "Up moves back");
        check(page(KeyEvent.KEYCODE_DPAD_LEFT) == HardwareTranslationCommitPolicy.PREVIOUS, "Left moves back");
        check(page(KeyEvent.KEYCODE_DPAD_DOWN) == HardwareTranslationCommitPolicy.NEXT, "Down moves on");
        check(page(KeyEvent.KEYCODE_DPAD_RIGHT) == HardwareTranslationCommitPolicy.NEXT, "Right moves on");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_DPAD_RIGHT, false, false, false, false,
                4, 3) == HardwareTranslationCommitPolicy.NEXT, "a held arrow keeps moving");
        check(page(KeyEvent.KEYCODE_MOVE_HOME) == HardwareTranslationCommitPolicy.FIRST, "Home goes to the first");
        check(page(KeyEvent.KEYCODE_MOVE_END) == HardwareTranslationCommitPolicy.LAST, "End goes to the last");

        // 其他键关页后照常处理：继续打字、退格。
        check(page(KeyEvent.KEYCODE_A) == HardwareTranslationCommitPolicy.PASS, "a letter leaves the page");
        check(page(KeyEvent.KEYCODE_DEL) == HardwareTranslationCommitPolicy.PASS, "Backspace leaves the page");
        check(page(KeyEvent.KEYCODE_TAB) == HardwareTranslationCommitPolicy.PASS, "Tab leaves the page");

        // 带修饰键的组合都关页后照常处理，Ctrl+Enter 因此会重新开页。
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_ENTER, false, true, false, false, 0, 3)
                == HardwareTranslationCommitPolicy.PASS, "Ctrl+Enter leaves the page to reopen it");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_1, true, false, false, false, 0, 3)
                == HardwareTranslationCommitPolicy.PASS, "Shift+1 is not a sense");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_1, false, false, true, false, 0, 3)
                == HardwareTranslationCommitPolicy.PASS, "Alt+1 is not a sense");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_SPACE, false, false, false, true, 0, 3)
                == HardwareTranslationCommitPolicy.PASS, "Meta+Space is not a commit");

        // 单按的修饰键留在页内：按下 Alt 准备下一个组合时页面不该先没了。
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_ALT_LEFT, false, false, true, false, 0, 3)
                == HardwareTranslationCommitPolicy.IGNORE, "a lone Alt keeps the page");
        check(HardwareTranslationCommitPolicy.pageKey(KeyEvent.KEYCODE_META_LEFT, false, false, false, true, 0,
                3) == HardwareTranslationCommitPolicy.IGNORE, "a lone Meta keeps the page");

        // 记作输入法的键（自动重复和抬起都不再交给应用）：上屏、关页和留在页内的键要记，移动高亮的键不记，否则服务在重复到达 pageKey 之前就把它吃掉，按住方向键只会动一格。
        check(HardwareTranslationCommitPolicy.holdsKey(0), "a picked sense holds its key");
        check(HardwareTranslationCommitPolicy.holdsKey(page(KeyEvent.KEYCODE_SPACE)), "Space holds its key");
        check(HardwareTranslationCommitPolicy.holdsKey(page(KeyEvent.KEYCODE_ESCAPE)), "Escape holds its key");
        check(HardwareTranslationCommitPolicy.holdsKey(page(KeyEvent.KEYCODE_9)),
            "a digit past the last sense holds its key");
        for (int arrow : new int[] {KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN,
                KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MOVE_HOME,
                KeyEvent.KEYCODE_MOVE_END}) {
            check(!HardwareTranslationCommitPolicy.holdsKey(page(arrow)), "a moving key repeats into the page");
        }
        check(!HardwareTranslationCommitPolicy.holdsKey(page(KeyEvent.KEYCODE_A)),
            "a key that leaves the page is the editor's");

        // 在线候选推进 generation 时：组字和高亮候选都没变，页面跟过去；任一变了或开页时没有记下，页面失效。
        check(HardwareTranslationCommitPolicy.pageSurvivesRefresh("wo de", "我的", "wo de", "我的"),
            "a late cloud or AI reply keeps the page");
        check(!HardwareTranslationCommitPolicy.pageSurvivesRefresh("wo de", "我的", "wo de", "窝的"),
            "a reply that moves the highlight closes the page");
        check(!HardwareTranslationCommitPolicy.pageSurvivesRefresh("wo de", "我的", "wo", "我的"),
            "a changed composition closes the page");
        check(!HardwareTranslationCommitPolicy.pageSurvivesRefresh("wo de", "我的", "wo de", ""),
            "a reply with no highlight closes the page");
        check(!HardwareTranslationCommitPolicy.pageSurvivesRefresh("", "", "", ""),
            "a page that recorded nothing does not survive");

        // 两套结果互不混淆：义项序号都不小于 0，页内常量都是负数，Ctrl+Enter 的结果都不是负数。
        int[] pageResults = {HardwareTranslationCommitPolicy.PASS, HardwareTranslationCommitPolicy.IGNORE,
            HardwareTranslationCommitPolicy.CLOSE, HardwareTranslationCommitPolicy.COMMIT_HIGHLIGHT,
            HardwareTranslationCommitPolicy.PREVIOUS, HardwareTranslationCommitPolicy.NEXT,
            HardwareTranslationCommitPolicy.FIRST, HardwareTranslationCommitPolicy.LAST};
        for (int index = 0; index < pageResults.length; index++) {
            check(pageResults[index] < 0, "page results cannot be read as a sense index");
            for (int other = index + 1; other < pageResults.length; other++)
                check(pageResults[index] != pageResults[other], "page results are distinct");
        }
        // 屏幕键盘长按「确认」（#6792）：去向和 Ctrl+Enter 相同，但没有译文时不接，长按当作点按照常确认组字。
        check(HardwareTranslationCommitPolicy.returnHold(true, 1) == HardwareTranslationCommitPolicy.COMMIT,
            "a held return commits the only sense");
        check(HardwareTranslationCommitPolicy.returnHold(true, 3) == HardwareTranslationCommitPolicy.OPEN_PAGE,
            "a held return opens the page for several senses");
        check(HardwareTranslationCommitPolicy.returnHold(true, 0) == HardwareTranslationCommitPolicy.NONE,
            "a held return without a gloss is a tap");
        check(HardwareTranslationCommitPolicy.returnHold(false, 2) == HardwareTranslationCommitPolicy.NONE,
            "a held return outside a gloss-inserting composition is a tap");
        for (int senses = 0; senses <= 9; senses++) {
            for (boolean eligible : new boolean[] {true, false}) {
                check(HardwareTranslationCommitPolicy.marksReturnKey(eligible, senses)
                        == (HardwareTranslationCommitPolicy.returnHold(eligible, senses)
                            != HardwareTranslationCommitPolicy.NONE),
                    "the return key is marked exactly when holding it commits a translation");
            }
        }
        System.out.println("Android hardware Ctrl+Enter and held return translation commit passed");
    }
}
