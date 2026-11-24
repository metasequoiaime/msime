import app.msime.android.JapaneseNineKeyLayout;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceSlot;
import app.msime.android.NineKeyCompositionActionPolicy;
import app.msime.android.NineKeyFaceDomains;
import app.msime.android.NineKeyLayout;
import app.msime.android.NineKeyShuangpinKeypad;
import app.msime.android.NumberPadPunctuation;
import app.msime.android.StrokeKeyboardLayout;
import app.msime.android.ZhuyinNineKeyLayout;
import java.util.List;

/** 九键键组的内容域：字母层九格（含 1 键组字时的分词、重输）和右列 ！、右上角数字 0–9；数字九宫格的数字、小数点和 ！ 的写法；九键双拼的数字和表里的简短提示；笔画键盘的笔画和 @#、0、句点、！；注音九键的声调、音键（注音和数字）和逗号句号；日语九键两层的键面。 */
public final class NineKeyFaceDomainsSmoke {
    private NineKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        KeyFaceDomain letters = NineKeyFaceDomains.letters();
        KeyFaceDomain digits = NineKeyFaceDomains.digits();
        for (List<NineKeyLayout.Key> row : NineKeyLayout.rows()) {
            for (NineKeyLayout.Key key : row) {
                check(letters.contains(KeyFaceSlot.MAIN, NineKeyLayout.face(key, false)), "缺九键键面 " + NineKeyLayout.face(key, false));
                for (String action : List.of(NineKeyCompositionActionPolicy.SEPARATE, NineKeyCompositionActionPolicy.REWRITE))
                    check(letters.contains(KeyFaceSlot.MAIN, NineKeyLayout.face(key, false, true, action)),
                        "缺组字时的键面 " + NineKeyLayout.face(key, false, true, action));
                check(letters.contains(KeyFaceSlot.TOP_RIGHT, NineKeyLayout.digitInput(key)), "缺右上角数字 " + key.digit());
                check(digits.contains(KeyFaceSlot.MAIN, NineKeyLayout.face(key, true)), "缺数字键面 " + key.digit());
            }
        }
        List<String> punctuation = NineKeyLayout.punctuation();
        String exclamation = punctuation.get(punctuation.size() - 1);
        check(letters.contains(KeyFaceSlot.MAIN, exclamation) && letters.contains(KeyFaceSlot.TOP_RIGHT, "0"),
            "缺右列的 ！ 和它滑出的 0");
        check(digits.contains(KeyFaceSlot.MAIN, "0") && digits.contains(KeyFaceSlot.MAIN, "."), "缺 0 或小数点");
        for (NumberPadPunctuation.Style style : NumberPadPunctuation.Style.values())
            check(digits.contains(KeyFaceSlot.MAIN, NumberPadPunctuation.face(exclamation, style)), "缺 ！ 的写法 " + style);
        check(!digits.has(KeyFaceSlot.TOP_RIGHT) && !digits.has(KeyFaceSlot.BOTTOM), "数字九宫格没有提示");

        List<NineKeyShuangpinKeypad.Face> faces = List.of(
            new NineKeyShuangpinKeypad.Face('2', List.of("b", "p"), List.of("a", "ia", "ua"), false),
            new NineKeyShuangpinKeypad.Face('8', List.of("zh"), List.of("uang"), false));
        KeyFaceDomain shuangpin = NineKeyFaceDomains.shuangpin(faces);
        for (char digit = '0'; digit <= '9'; digit++)
            check(shuangpin.contains(KeyFaceSlot.MAIN, String.valueOf(digit)), "缺双拼数字 " + digit);
        for (NineKeyShuangpinKeypad.Face face : faces)
            check(shuangpin.contains(KeyFaceSlot.BOTTOM, NineKeyShuangpinKeypad.compactHint(face)),
                "缺表里的提示 " + NineKeyShuangpinKeypad.compactHint(face));
        check(!shuangpin.has(KeyFaceSlot.TOP_RIGHT), "九键双拼没有右上角");

        KeyFaceDomain strokes = NineKeyFaceDomains.strokes();
        for (List<StrokeKeyboardLayout.Key> row : StrokeKeyboardLayout.rows())
            for (StrokeKeyboardLayout.Key key : row)
                check(strokes.contains(KeyFaceSlot.MAIN, StrokeKeyboardLayout.face(key)), "缺笔画 " + key.name());
        KeyFaceDomain strokeKeys = NineKeyFaceDomains.strokeKeys();
        for (String face : List.of("@#", "0", "。", ".", exclamation))
            check(strokeKeys.contains(KeyFaceSlot.MAIN, face), "笔画键盘的其他键缺 " + face);
        check(!strokes.has(KeyFaceSlot.TOP_RIGHT) && !strokeKeys.has(KeyFaceSlot.TOP_RIGHT), "笔画键盘没有右上角");

        KeyFaceDomain tones = NineKeyFaceDomains.zhuyinTones();
        for (ZhuyinNineKeyLayout.Tone tone : ZhuyinNineKeyLayout.tones())
            check(tones.contains(KeyFaceSlot.MAIN, tone.face()), "缺注音声调 " + tone.label());
        KeyFaceDomain sounds = NineKeyFaceDomains.zhuyinSounds();
        List<ZhuyinNineKeyLayout.Key> soundKeys = new java.util.ArrayList<>();
        for (List<ZhuyinNineKeyLayout.Key> row : ZhuyinNineKeyLayout.rows()) soundKeys.addAll(row);
        soundKeys.add(ZhuyinNineKeyLayout.zero());
        for (ZhuyinNineKeyLayout.Key key : soundKeys)
            check(sounds.contains(KeyFaceSlot.MAIN, key.symbols()) && sounds.contains(KeyFaceSlot.BOTTOM, String.valueOf(key.digit())),
                "缺注音音键 " + key.digit());
        KeyFaceDomain marks = NineKeyFaceDomains.zhuyinMarks();
        for (String face : List.of("，", "。", ",", "."))
            check(marks.contains(KeyFaceSlot.MAIN, face), "注音九键的逗号句号缺 " + face);
        check(marks.contains(KeyFaceSlot.MAIN, "ㄅ"), "注音九键的逗号句号缺基线参照");

        for (boolean symbols : new boolean[] {false, true}) {
            KeyFaceDomain kana = NineKeyFaceDomains.japaneseKana(symbols);
            for (JapaneseNineKeyLayout.Key key : symbols ? JapaneseNineKeyLayout.digitKeys() : JapaneseNineKeyLayout.keys())
                check(kana.contains(KeyFaceSlot.MAIN, key.kana().get(0)), "日语九键缺键面 " + key.kana().get(0));
        }
        check(!NineKeyFaceDomains.japaneseKana(false).contains(KeyFaceSlot.MAIN, "1"), "日语九键假名组不应有数字");
        System.out.println("NineKeyFaceDomainsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
