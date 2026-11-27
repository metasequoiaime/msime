import app.msime.android.KeyPressIds;
import app.msime.android.ZhuyinNineKeyLayout;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** 注音 9 键：十个音键的注音分组（US 6,009,444 FIG. 1）、键面、发送的数字，五个声调键发送的字母，以及统计 id。 */
public final class ZhuyinNineKeyLayoutSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        List<List<ZhuyinNineKeyLayout.Key>> rows = ZhuyinNineKeyLayout.rows();
        check(rows.size() == 3 && rows.stream().allMatch(row -> row.size() == 3), "the grid is 3 by 3 above 0");
        List<ZhuyinNineKeyLayout.Key> keys = new ArrayList<>();
        for (List<ZhuyinNineKeyLayout.Key> row : rows) keys.addAll(row);
        keys.add(ZhuyinNineKeyLayout.zero());
        List<String> groups = new ArrayList<>();
        StringBuilder inputs = new StringBuilder();
        for (ZhuyinNineKeyLayout.Key key : keys) {
            groups.add(key.digit() + key.symbols());
            inputs.append(key.input());
        }
        // The grouping is fixed by the patent figure; the Engine's KEYPAD table carries the same ten groups.
        check(groups.equals(List.of("1ㄅㄆㄇㄈ", "2ㄉㄊㄋㄌ", "3ㄍㄎㄏ", "4ㄐㄑㄒ", "5ㄓㄔㄕㄖ",
            "6ㄗㄘㄙ", "7ㄚㄛㄜㄝ", "8ㄧㄨㄩㄦ", "9ㄞㄟㄠㄡ", "0ㄢㄣㄤㄥ")), "the patent grouping: " + groups);
        check("1234567890".contentEquals(inputs), "each key sends its ASCII digit: " + inputs);
        Set<Integer> bopomofo = new HashSet<>();
        int total = 0;
        for (ZhuyinNineKeyLayout.Key key : keys) {
            for (int index = 0; index < key.symbols().length(); index++) {
                char symbol = key.symbols().charAt(index);
                check(symbol >= 0x3105 && symbol <= 0x3129, "a key face holds bopomofo only: " + symbol);
                bopomofo.add((int) symbol);
                total++;
            }
        }
        check(total == 37 && bopomofo.size() == 37, "all 37 bopomofo, each on exactly one key");

        ZhuyinNineKeyLayout.Key one = rows.get(0).get(0);
        check("ㄅㄆㄇㄈ\n1".equals(ZhuyinNineKeyLayout.face(one)), "a key prints its bopomofo over its digit");
        check("ㄢㄣㄤㄥ\n0".equals(ZhuyinNineKeyLayout.face(ZhuyinNineKeyLayout.zero())), "0 prints its finals");
        check("注音 1 ㄅㄆㄇㄈ".equals(ZhuyinNineKeyLayout.accessibilityLabel(one)), "spoken label names digit and group");
        for (ZhuyinNineKeyLayout.Key key : keys) {
            String id = ZhuyinNineKeyLayout.keyId(key);
            check(KeyPressIds.isKnown(id) && id.equals("Nine" + key.digit()), "a sound key counts as its nine-key cell: " + id);
        }

        List<ZhuyinNineKeyLayout.Tone> tones = ZhuyinNineKeyLayout.tones();
        StringBuilder toneInputs = new StringBuilder();
        List<String> faces = new ArrayList<>();
        for (ZhuyinNineKeyLayout.Tone tone : tones) {
            toneInputs.append(tone.input());
            faces.add(tone.face());
            check(KeyPressIds.isKnown(tone.keyId()), "a tone key id is in the store's whitelist: " + tone.keyId());
            check(tone.face().length() == 1, "one UTF-16 unit per tone face: " + tone.face());
        }
        // The cross-layer contract: the Engine's nine-key editor reads z x c v b as ˉ ˊ ˇ ˋ ˙.
        check("zxcvb".contentEquals(toneInputs), "the tone keys send z x c v b: " + toneInputs);
        check(faces.equals(List.of("ˉ", "ˊ", "ˇ", "ˋ", "˙")), "the tone faces: " + faces);
        check("注音 一声".equals(ZhuyinNineKeyLayout.accessibilityLabel(tones.get(0)))
            && "注音 轻声".equals(ZhuyinNineKeyLayout.accessibilityLabel(tones.get(4))), "spoken tone labels");
        // A tone counts on the Dachen key that types the same tone, so the heatmap merges both keyboards.
        check(tones.stream().map(ZhuyinNineKeyLayout.Tone::keyId).toList().equals(
            List.of("Space", "Digit6", "Digit3", "Digit4", "Digit7")), "tone ids follow the Dachen tone keys");

        check("注".equals(ZhuyinNineKeyLayout.LAYER_TITLE), "the symbol page returns to 注");

        boolean rejected = false;
        try { ZhuyinNineKeyLayout.face(null); } catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "a missing key is a programming error");
        System.out.println("Android Zhuyin nine-key: patent grouping, faces, digits, tone letters, and key ids passed");
    }
}
