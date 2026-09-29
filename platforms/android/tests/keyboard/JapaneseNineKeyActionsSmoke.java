import app.msime.android.JapaneseNineKeyActions;

public final class JapaneseNineKeyActionsSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(JapaneseNineKeyActions.spaceTitle(false).equals("空白"));
        check(JapaneseNineKeyActions.spaceTitle(true).equals("変換"));
        check(JapaneseNineKeyActions.returnTitle(false).equals("改行"));
        check(JapaneseNineKeyActions.returnTitle(true).equals("確定"));
        System.out.println("Android Japanese side-key labels passed");
    }
}
