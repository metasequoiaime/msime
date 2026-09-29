import app.msime.android.KeyboardLayout;
import app.msime.android.LetterKeyFacePolicy;
import java.util.List;

public final class KeyboardLayoutSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        List<List<String>> letters = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS);
        check(letters.size() == 3);
        check(letters.get(0).equals(List.of("q", "w", "e", "r", "t", "y", "u", "i", "o", "p")));
        check(letters.get(2).equals(List.of("z", "x", "c", "v", "b", "n", "m")));

        // What a key sends and what it shows are answered separately. A Chinese keyboard draws its
        // 26 keys in caps, and the engine still has to receive the lowercase letter: sending the
        // drawn form made the engine decline it and the host commit `N` literally.
        for (List<String> row : letters) {
            for (String key : row) {
                check(key.equals(key.toLowerCase(java.util.Locale.ROOT)));
                check(LetterKeyFacePolicy.face(key, true, false, false)
                    .equals(key.toUpperCase(java.util.Locale.ROOT)));
                check(LetterKeyFacePolicy.face(key, false, false, false).equals(key));
            }
        }

        List<List<String>> symbols = KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS);
        check(symbols.size() == 3);
        check(symbols.get(0).equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")));
        check(symbols.get(1).contains("\""));
        check(symbols.get(2).equals(List.of("(", ")", "[", "]", "<", ">", "\\", "-", "_", "=")));

        check(KeyboardLayout.resolveTouchLayout(false, false, 0, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, true, 0, "nine_key")
            == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, true, 3, "nine_key")
            == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 3, "nine_key")
            == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(true, true, 0, "handwriting")
            == KeyboardLayout.HANDWRITING_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 3, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);

        System.out.println("Android keyboard layers: canonical keys, faces and symbol layouts passed");
    }
}
