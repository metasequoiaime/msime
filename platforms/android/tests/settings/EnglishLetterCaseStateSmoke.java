import app.msime.android.EnglishLetterCaseState;

public final class EnglishLetterCaseStateSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        EnglishLetterCaseState state = new EnglishLetterCaseState();
        check(state.mode() == EnglishLetterCaseState.Mode.LOWERCASE);
        check(state.accessibilityLabel(false).equals("切换到英文大写"));
        check(state.accessibilityValue().equals("关闭"));

        state.toggle(1_000);
        check(state.mode() == EnglishLetterCaseState.Mode.SHIFTED);
        check(state.accessibilityValue().equals("下一字母"));
        check(state.consumeLetter());
        check(state.mode() == EnglishLetterCaseState.Mode.LOWERCASE);

        state.toggle(2_000);
        state.toggle(2_350);
        check(state.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
        check(state.keyText().equals("⇪"));
        check(state.accessibilityLabel(true).equals("大写锁定"));
        check(state.accessibilityValue().equals("开启"));
        check(!state.consumeLetter());
        check(!state.applyAutomatic(false));
        check(state.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
        state.toggle(2_400);
        check(state.mode() == EnglishLetterCaseState.Mode.LOWERCASE);

        check(state.applyAutomatic(true));
        check(state.isAutomatic());
        check(state.accessibilityValue().equals("自动开启"));
        check(state.consumeLetter());
        check(!state.isAutomatic());
        state.toggle(3_000);
        state.toggle(3_351);
        check(state.mode() == EnglishLetterCaseState.Mode.LOWERCASE);

        // 句首自动大写时双击：第一下关掉大写，第二下锁定。
        EnglishLetterCaseState sentenceStart = new EnglishLetterCaseState();
        sentenceStart.applyAutomatic(true);
        sentenceStart.toggle(4_000);
        check(sentenceStart.mode() == EnglishLetterCaseState.Mode.LOWERCASE);
        sentenceStart.toggle(4_200);
        check(sentenceStart.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);

        // 两次点按之间编辑器回报光标位置，自动大写重新计算，双击仍然锁定。
        EnglishLetterCaseState echoed = new EnglishLetterCaseState();
        echoed.toggle(5_000);
        check(echoed.isPressedShift());
        echoed.applyAutomatic(true);
        echoed.toggle(5_200);
        check(echoed.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK);
        check(!echoed.isPressedShift());

        // 锁定时点一下回到小写，紧接着的一下只是单次大写，不会又锁上。
        echoed.toggle(5_300);
        check(echoed.mode() == EnglishLetterCaseState.Mode.LOWERCASE);
        echoed.toggle(5_400);
        check(echoed.mode() == EnglishLetterCaseState.Mode.SHIFTED);
        check(echoed.isPressedShift());

        // 自动大写不是手动按下的。
        EnglishLetterCaseState automatic = new EnglishLetterCaseState();
        automatic.applyAutomatic(true);
        check(!automatic.isPressedShift());
        System.out.println("Android English letter case: one-shot Shift, Caps Lock and automatic state passed");
    }
}
