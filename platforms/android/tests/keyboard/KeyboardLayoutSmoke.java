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
        check(KeyboardLayout.resolveTouchLayout(false, false, 4, "twenty_six_key")
            == KeyboardLayout.KOREAN_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 4, "nine_key")
            == KeyboardLayout.KOREAN_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(true, false, 4, "handwriting")
            == KeyboardLayout.KOREAN_LAYOUT);
        check(KeyboardLayout.carriesLetterCase(KeyboardLayout.STANDARD_TOUCH_LAYOUT)
            && KeyboardLayout.carriesLetterCase(KeyboardLayout.KOREAN_LAYOUT)
            && !KeyboardLayout.carriesLetterCase(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT)
            && !KeyboardLayout.carriesLetterCase(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT)
            && !KeyboardLayout.carriesLetterCase(KeyboardLayout.HANDWRITING_LAYOUT)
            && !KeyboardLayout.carriesLetterCase(KeyboardLayout.ZHUYIN_LAYOUT));
        // Zhuyin is Dachen only; Cantonese and Vietnamese keep the 26 QWERTY keys, and Vietnamese keeps their case.
        check(KeyboardLayout.resolveTouchLayout(false, false, 6, "twenty_six_key")
            == KeyboardLayout.ZHUYIN_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(true, true, 6, "handwriting")
            == KeyboardLayout.ZHUYIN_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 5, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 7, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        // Stroke draws its own keypad over a 26-key or nine-key preference and yields only to handwriting.
        check(KeyboardLayout.resolveTouchLayout(false, false, 8, "twenty_six_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, true, 8, "nine_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 8, "nine_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(true, false, 8, "handwriting")
            == KeyboardLayout.HANDWRITING_LAYOUT);
        check(!KeyboardLayout.carriesLetterCase(KeyboardLayout.STROKE_LAYOUT));
        check(KeyboardLayout.STROKE_LAYOUT != KeyboardLayout.ZHUYIN_LAYOUT
            && KeyboardLayout.STROKE_LAYOUT != KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT);
        List<List<String>> dachen = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS, KeyboardLayout.ZHUYIN_LAYOUT);
        check(dachen.size() == 4 && dachen.get(0).size() == 11 && "1".equals(dachen.get(0).get(0))
            && "-".equals(dachen.get(0).get(10)) && ";".equals(dachen.get(2).get(9)) && "/".equals(dachen.get(3).get(9)));
        check(KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.ZHUYIN_LAYOUT)
            == KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS));
        check(KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT)
            == KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS));

        System.out.println("Android keyboard layers: canonical keys, faces and symbol layouts passed");
    }
}
