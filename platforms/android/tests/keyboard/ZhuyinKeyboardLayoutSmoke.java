import app.msime.android.ZhuyinKeyboardLayout;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

public final class ZhuyinKeyboardLayoutSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        List<List<String>> rows = ZhuyinKeyboardLayout.rows();
        check(rows.size() == 4, "Dachen has four rows");
        check(rows.get(0).equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-")),
            "the digit row carries ㄅㄉ, the tones and ㄓㄚㄞㄢㄦ");
        check(rows.get(1).size() == 10 && rows.get(2).size() == 10 && rows.get(3).size() == 10,
            "the letter rows have ten keys each");
        Set<String> faces = new HashSet<>();
        int keys = 0;
        for (List<String> row : rows) {
            for (String key : row) {
                keys++;
                check(key.length() == 1 && key.charAt(0) >= 32 && key.charAt(0) < 127,
                    "every key sends one ASCII character");
                check(!ZhuyinKeyboardLayout.face(key).equals(key), "every key has a bopomofo face: " + key);
                faces.add(ZhuyinKeyboardLayout.face(key));
            }
        }
        check(keys == 41 && faces.size() == 41, "41 keys, each with its own face");
        // 37 bopomofo and four tone marks; Space is the first tone.
        int bopomofo = 0;
        for (String face : faces) if (face.charAt(0) >= 'ㄅ' && face.charAt(0) <= 'ㄩ') bopomofo++;
        check(bopomofo == 37, "every bopomofo symbol is on a key");
        check("ㄅ".equals(ZhuyinKeyboardLayout.face("1")) && "ㄆ".equals(ZhuyinKeyboardLayout.face("q"))
            && "ㄇ".equals(ZhuyinKeyboardLayout.face("a")) && "ㄈ".equals(ZhuyinKeyboardLayout.face("z"))
            && "ㄤ".equals(ZhuyinKeyboardLayout.face(";")) && "ㄥ".equals(ZhuyinKeyboardLayout.face("/"))
            && "ㄦ".equals(ZhuyinKeyboardLayout.face("-")) && "ㄩ".equals(ZhuyinKeyboardLayout.face("m")),
            "keys sit where a Dachen keyboard prints them");
        check("ˊ".equals(ZhuyinKeyboardLayout.face("6")) && "ˇ".equals(ZhuyinKeyboardLayout.face("3"))
            && "ˋ".equals(ZhuyinKeyboardLayout.face("4")) && "˙".equals(ZhuyinKeyboardLayout.face("7")),
            "the tone keys");
        check("注音 二声".equals(ZhuyinKeyboardLayout.accessibilityLabel("6"))
            && "注音 轻声".equals(ZhuyinKeyboardLayout.accessibilityLabel("7"))
            && "注音 ㄅ".equals(ZhuyinKeyboardLayout.accessibilityLabel("1")),
            "tone keys say the tone, the others the symbol");
        check("@".equals(ZhuyinKeyboardLayout.face("@")) && ZhuyinKeyboardLayout.face(null).isEmpty()
            && "注音符号".equals(ZhuyinKeyboardLayout.accessibilityLabel(null)),
            "a key outside the layout keeps its own face");
        for (char key : "1234567890,./;-".toCharArray()) {
            check(ZhuyinKeyboardLayout.claimsSymbol(String.valueOf(key)), "Dachen claims " + key);
        }
        check(!ZhuyinKeyboardLayout.claimsSymbol("?") && !ZhuyinKeyboardLayout.claimsSymbol("!")
            && !ZhuyinKeyboardLayout.claimsSymbol(" ") && !ZhuyinKeyboardLayout.claimsSymbol("10")
            && !ZhuyinKeyboardLayout.claimsSymbol(null), "other symbols go to the Engine as punctuation");
        check("注".equals(ZhuyinKeyboardLayout.LAYER_TITLE), "the symbol page returns to 注");
        System.out.println("Android Zhuyin keyboard: Dachen rows, faces, tones and claimed symbols passed");
    }
}
