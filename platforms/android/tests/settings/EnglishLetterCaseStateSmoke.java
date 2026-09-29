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
        System.out.println("Android English letter case: one-shot Shift, Caps Lock and automatic state passed");
    }
}
