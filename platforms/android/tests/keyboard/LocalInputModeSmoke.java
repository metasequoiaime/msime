import app.msime.android.LocalInputMode;

public final class LocalInputModeSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(LocalInputMode.values().length == 9);
        check(LocalInputMode.values()[0] == LocalInputMode.UNICODE);
        check(LocalInputMode.values()[1] == LocalInputMode.DATE_TIME);
        check(LocalInputMode.values()[2] == LocalInputMode.SUPER_JIANPIN);
        check(LocalInputMode.values()[3] == LocalInputMode.QUICK_PHRASE);
        check(LocalInputMode.values()[4] == LocalInputMode.TEMPORARY_ENGLISH);
        check(LocalInputMode.values()[5] == LocalInputMode.EMOJI);
        check(LocalInputMode.values()[6] == LocalInputMode.KAOMOJI);
        check(LocalInputMode.values()[7] == LocalInputMode.TEMPORARY_JAPANESE);
        check(LocalInputMode.values()[8] == LocalInputMode.CHAIZI);
        check(LocalInputMode.UNICODE.trigger().equals("U"));
        check(LocalInputMode.UNICODE.title().equals("Unicode 码点"));
        check(LocalInputMode.UNICODE.preferenceKey().equals("unicode"));
        check(LocalInputMode.DATE_TIME.trigger().equals("T"));
        check(LocalInputMode.DATE_TIME.title().equals("日期时间"));
        check(LocalInputMode.QUICK_PHRASE.trigger().equals("K"));
        check(LocalInputMode.EMOJI.trigger().equals("E"));
        check(LocalInputMode.EMOJI.title().equals("表情"));
        check(LocalInputMode.KAOMOJI.trigger().equals("M"));
        check(LocalInputMode.SUPER_JIANPIN.trigger().equals("J"));
        check(LocalInputMode.TEMPORARY_ENGLISH.trigger().equals("Y"));
        check(LocalInputMode.TEMPORARY_ENGLISH.title().equals("英文补全"));
        check(LocalInputMode.TEMPORARY_JAPANESE.trigger().equals("R"));
        check(LocalInputMode.CHAIZI.trigger().equals("C"));
        check(LocalInputMode.CHAIZI.title().equals("拆字"));
        check(LocalInputMode.CHAIZI.preferenceKey().equals("chaizi"));
        System.out.println("Android local input modes: Apple triggers and preference keys passed");
    }
}
