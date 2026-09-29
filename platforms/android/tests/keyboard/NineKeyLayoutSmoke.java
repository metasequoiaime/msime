import app.msime.android.NineKeyLayout;
import java.util.List;

public final class NineKeyLayoutSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(NineKeyLayout.rows().size() == 3);
        check(NineKeyLayout.rows().stream().allMatch(row -> row.size() == 3));
        check(NineKeyLayout.rows().stream().flatMap(List::stream)
            .map(NineKeyLayout.Key::label).toList().equals(
                List.of("分词", "ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ")));
        check(NineKeyLayout.rows().stream().flatMap(List::stream)
            .map(NineKeyLayout.Key::input).toList().equals(
                List.of('\'', '2', '3', '4', '5', '6', '7', '8', '9')));
        check(NineKeyLayout.rows().stream().flatMap(List::stream)
            .map(NineKeyLayout.Key::digit).toList().equals(
                List.of(1, 2, 3, 4, 5, 6, 7, 8, 9)));
        check(NineKeyLayout.punctuation().equals(List.of("，", "。", "？", "！")));

        List<NineKeyLayout.Key> keys = NineKeyLayout.rows().stream().flatMap(List::stream).toList();
        NineKeyLayout.Key separator = keys.get(0);
        NineKeyLayout.Key letters = keys.get(1);
        check(NineKeyLayout.face(separator, false).equals("分词"));
        check(NineKeyLayout.face(letters, false).equals("ABC"));
        check(NineKeyLayout.description(separator, false).equals("拼音分词"));
        check(NineKeyLayout.description(letters, false).equals("2 ABC"));
        // The digit layer prints the number the key carries, including key 1, whose Engine input is
        // the pinyin separator rather than a number.
        check(NineKeyLayout.face(separator, true).equals("1"));
        check(NineKeyLayout.face(letters, true).equals("2"));
        check(NineKeyLayout.description(separator, true).equals("数字 1"));
        check(NineKeyLayout.description(letters, true).equals("数字 2"));
        check(NineKeyLayout.digitInput(separator).equals("1"));
        check(NineKeyLayout.digitInput(keys.get(8)).equals("9"));
        for (NineKeyLayout.Key key : keys) {
            check(NineKeyLayout.face(key, true).equals(NineKeyLayout.digitInput(key)));
            check(NineKeyLayout.description(key, true).equals("数字 " + key.digit()));
        }
        try {
            NineKeyLayout.face(null, true);
            throw new AssertionError();
        } catch (IllegalArgumentException expected) { }
        try {
            NineKeyLayout.description(null, false);
            throw new AssertionError();
        } catch (IllegalArgumentException expected) { }
        try {
            NineKeyLayout.digitInput(null);
            throw new AssertionError();
        } catch (IllegalArgumentException expected) { }

        try {
            NineKeyLayout.rows().get(0).add(new NineKeyLayout.Key(0, "bad", '0', "bad"));
            throw new AssertionError();
        } catch (UnsupportedOperationException expected) { }
        System.out.println(
            "Android nine-key layout: grid inputs, labels, digit layer and punctuation passed");
    }
}
