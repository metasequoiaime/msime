import app.msime.android.KeyboardInputContext;

public final class KeyboardInputContextSmoke {
    static void check(Boolean actual, Boolean expected) {
        if (actual == null ? expected != null : !actual.equals(expected)) throw new AssertionError();
    }

    public static void main(String[] args) {
        KeyboardInputContext context = new KeyboardInputContext();
        check(context.englishOverride(false, 1, false), null);
        check(context.englishOverride(true, 2, false), true);
        check(context.englishOverride(true, 2, false), null);
        check(context.englishOverride(true, 3, false), true);
        check(context.englishOverride(false, 1, true), false);
        check(context.englishOverride(true, 2, false), true);
        check(context.englishOverride(false, 1, true), false);
        check(context.englishOverride(false, 4, false), null);

        KeyboardInputContext alreadyEnglish = new KeyboardInputContext();
        check(alreadyEnglish.englishOverride(false, 10, true), null);
        check(alreadyEnglish.englishOverride(true, 11, true), true);
        check(alreadyEnglish.englishOverride(false, 12, true), true);
        System.out.println("Android field language override policy passed");
    }
}
