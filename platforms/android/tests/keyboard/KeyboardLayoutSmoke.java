import app.msime.android.InputSchemeTraits;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardLayout;
import app.msime.android.LetterKeyFacePolicy;
import java.util.List;

public final class KeyboardLayoutSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 入口只改变数字页：中文常规布局复用网格，独立语言布局与本地模式保持原契约。
        for (int layout : new int[] {0, 1, 3, 6, 7, 8}) {
            check(KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.SYMBOLS, layout, true, false));
            check(!KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.LETTERS, layout, true, false));
        }
        for (int layout : new int[] {2, 4, 5})
            check(!KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.SYMBOLS, layout, true, false));
        check(!KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.SYMBOLS, 0, false, false));
        check(!KeyboardLayout.drawsNumberPad(KeyboardLayout.Layer.SYMBOLS, 0, true, true));
        // 日文罗马字同样画 26 键，但 123 保留自己的符号层，与 iOS、HarmonyOS 一致。
        check(!KeyboardLayout.numberPadLanguage(InputSchemeTraits.JAPANESE, false));
        check(!KeyboardLayout.numberPadLanguage(InputSchemeTraits.VIETNAMESE, false));
        check(!KeyboardLayout.numberPadLanguage(InputSchemeTraits.TIBETAN, false));
        check(!KeyboardLayout.numberPadLanguage(InputSchemeTraits.QUANPIN, true));
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN, InputSchemeTraits.WUBI,
                InputSchemeTraits.CANTONESE, InputSchemeTraits.ZHUYIN, InputSchemeTraits.STROKE, -1})
            check(KeyboardLayout.numberPadLanguage(scheme, false));
        List<List<String>> letters = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS);
        check(letters.size() == 3);
        check(letters.get(0).equals(List.of("q", "w", "e", "r", "t", "y", "u", "i", "o", "p")));
        check(letters.get(2).equals(List.of("z", "x", "c", "v", "b", "n", "m")));

        // 键发出什么和键面画什么分开回答：发给 Engine 的一律是小写字母；中文模式键面画大写，英文平时画小写、Shift 才画大写。
        for (List<String> row : letters) {
            for (String key : row) {
                String upper = key.toUpperCase(java.util.Locale.ROOT);
                check(key.equals(key.toLowerCase(java.util.Locale.ROOT)));
                check(LetterKeyFacePolicy.face(key, true, false, false, false).equals(upper));
                check(LetterKeyFacePolicy.face(key, false, false, false, true).equals(upper));
                check(LetterKeyFacePolicy.face(key, false, false, false, false).equals(key));
            }
        }

        // #6022：数字行是 1–0，只画在 26 键和韩文键盘的字母层，设置关着时不画。
        check(KeyboardLayout.NUMBER_ROW.equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")));
        int portrait = 760;
        check(KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, portrait));
        check(KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, KeyboardLayout.KOREAN_LAYOUT, portrait));
        check(!KeyboardLayout.drawsNumberRow(false, KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, portrait));
        check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, portrait));
        for (int grid : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT,
                KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, KeyboardLayout.FOURTEEN_KEY_LAYOUT})
            check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, grid, portrait));
        // 横屏手机（360 / 411 / 440 dp 宽的手机横过来，窗口约 336–416 dp 高）不画，免得工具栏或底行被挤出窗口；平板横屏（约 552 dp 起）照画；窗口高度未知时不按高度拦。
        for (int landscapePhone : new int[] {336, 387, 416, KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP - 1})
            check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, landscapePhone));
        for (int tall : new int[] {KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP, 552, 0})
            check(KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, tall));

        // #8210：第五排资格只继承源字母页；符号页实际增加可用字符。
        for (int layout = 0; layout <= 8; layout++) {
            for (int height : new int[] {0, 416, 480, 760}) {
                boolean eligible = KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout, height);
                check(KeyboardLayout.drawsSymbolExtraRow(true, KeyboardLayout.Layer.SYMBOLS, layout, height) == eligible);
                check(!KeyboardLayout.drawsSymbolExtraRow(false, KeyboardLayout.Layer.SYMBOLS, layout, height));
                check(!KeyboardLayout.drawsSymbolExtraRow(true, KeyboardLayout.Layer.LETTERS, layout, height));
            }
        }
        check(KeyboardLayout.symbolExtraRow(true, false, true).equals(KeyboardLayout.NUMBER_ROW));
        check(KeyboardLayout.symbolExtraRow(false, false, true).equals(
            List.of("[", "]", "{", "}", "#", "%", "^", "*", "+", "=")));
        for (boolean chinese : new boolean[] {true, false}) {
            List<String> extra = KeyboardLayout.symbolExtraRow(false, true, chinese);
            check(extra.size() == 10 && new java.util.HashSet<>(extra).size() == 10);
            check(extra.get(0).equals(chinese ? "（" : "("));
            check(extra.contains("‰") && extra.contains("²"));
        }

        // 同属 26 键的双拼和直输场景仍是 Shift，不能仅凭布局决定分词动作。
        check(KeyboardLayout.usesQuanpinSeparator(true, true,
            KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, "none"));
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            check(KeyboardLayout.usesQuanpinSeparator(scheme == KeyboardScheme.QUANPIN, true, KeyboardLayout.Layer.LETTERS,
                KeyboardLayout.STANDARD_TOUCH_LAYOUT, "none") == (scheme == KeyboardScheme.QUANPIN));
        }
        check(!KeyboardLayout.usesQuanpinSeparator(true, false,
            KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, "none"));
        check(!KeyboardLayout.usesQuanpinSeparator(true, true,
            KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, "none"));
        check(!KeyboardLayout.usesQuanpinSeparator(true, true,
            KeyboardLayout.Layer.LETTERS, KeyboardLayout.FOURTEEN_KEY_LAYOUT, "none"));
        check(!KeyboardLayout.usesQuanpinSeparator(true, true,
            KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT, "calculator"));

        // 新设计的 123 层（中文）。
        List<List<KeyboardLayout.LayerKey>> number = KeyboardLayout.numberLayer(true);
        check(number.size() == 4);
        check(texts(number.get(0)).equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")));
        check(texts(number.get(1)).equals(List.of("-", "/", "：", "；", "（", "）", "¥", "@", "“", "”")));
        check(texts(number.get(2)).equals(List.of("#+=", "。", "，", "、", "？", "！", "⌫")));
        check(number.get(2).get(0).description().equals("更多符号"));
        check(number.get(2).get(6).description().equals("删除"));
        List<KeyboardLayout.LayerKey> bottom = number.get(3);
        check(texts(bottom).equals(List.of("拼音", "😀", "中标", "空格", "换行")));
        check(bottom.get(0).description().equals("切换到字母键盘")
            && bottom.get(0).kind() == KeyboardLayout.LayerKeyKind.LETTERS);
        check(bottom.get(1).kind() == KeyboardLayout.LayerKeyKind.EMOJI);
        check(bottom.get(2).description().equals("切换到英文标点")
            && bottom.get(2).kind() == KeyboardLayout.LayerKeyKind.PUNCTUATION_TOGGLE);
        float total = 0;
        for (KeyboardLayout.LayerKey key : bottom) total += key.weight();
        check(Math.abs(total - 10.2f) < 1e-4);
        // 直角引号样式下中文 123 层的两个引号键是「」，其余不变；中文标点关着时仍是英文版。
        List<List<KeyboardLayout.LayerKey>> corner = KeyboardLayout.numberLayer(true, true, true);
        check(texts(corner.get(1)).equals(List.of("-", "/", "：", "；", "（", "）", "¥", "@", "「", "」")));
        check(texts(corner.get(2)).equals(texts(number.get(2))));
        check(texts(KeyboardLayout.numberLayer(true, false, true).get(1))
            .equals(texts(KeyboardLayout.numberLayer(false).get(1))));
        // 英文 123 层是 ASCII 版，底行返回键为 ABC。
        List<List<KeyboardLayout.LayerKey>> english = KeyboardLayout.numberLayer(false);
        check(texts(english.get(1)).equals(List.of("-", "/", ":", ";", "(", ")", "$", "@", "\"", "'")));
        check(texts(english.get(2)).equals(List.of("#+=", ".", ",", "?", "!", "…", "⌫")));
        check(english.get(3).get(0).text().equals("ABC")
            && english.get(3).get(0).description().equals("切换到字母键盘"));
        check(english.get(3).get(2).text().equals("英标")
            && english.get(3).get(2).description().equals("切换到中文标点"));
        // 中文模式关掉中文标点：标点和符号是半角英文，返回键仍是「拼音」。0.2.0 起这一层只看中英文模式，关掉中文标点照样上屏「，」。
        List<List<KeyboardLayout.LayerKey>> halfWidth = KeyboardLayout.numberLayer(true, false);
        check(texts(halfWidth.get(1)).equals(texts(english.get(1))));
        check(texts(halfWidth.get(2)).equals(texts(english.get(2))));
        check(halfWidth.get(3).get(0).text().equals("拼音"));
        check(texts(KeyboardLayout.moreSymbolLayer(true, false).get(1))
            .equals(texts(KeyboardLayout.moreSymbolLayer(false).get(1))));
        check(KeyboardLayout.moreSymbolLayer(true, false).get(3).get(0).text().equals("拼音"));
        // 数字页根据顶部入口换键，符号和表情互斥，宽度与其余按键保持一致。
        for (boolean chinese : List.of(false, true)) {
            for (boolean punctuation : List.of(false, true)) {
                List<KeyboardLayout.LayerKey> fallback = KeyboardLayout.numberLayer(chinese, punctuation, true, false).get(3);
                List<KeyboardLayout.LayerKey> toolbar = KeyboardLayout.numberLayer(chinese, punctuation, true, true).get(3);
                check(fallback.get(1).kind() == KeyboardLayout.LayerKeyKind.EMOJI);
                check(toolbar.get(1).kind() == KeyboardLayout.LayerKeyKind.SYMBOL_PANEL);
                check(toolbar.get(1).text().equals("符号"));
                check(toolbar.stream().noneMatch(key -> key.kind() == KeyboardLayout.LayerKeyKind.EMOJI));
                check(toolbar.get(1).weight() == fallback.get(1).weight());
                for (int index : List.of(0, 2, 3, 4)) check(toolbar.get(index).equals(fallback.get(index)));
            }
        }
        // #+= 层：左下原表情位是打开符号面板的「符号」键，切换键翻成 123。
        List<List<KeyboardLayout.LayerKey>> more = KeyboardLayout.moreSymbolLayer(true);
        check(texts(more.get(0)).equals(List.of("[", "]", "{", "}", "#", "%", "^", "*", "+", "=")));
        check(texts(more.get(1)).equals(List.of("_", "\\", "|", "~", "《", "》", "€", "&", "·", "…")));
        check(more.get(2).get(0).text().equals("123")
            && more.get(2).get(0).description().equals("切换到数字和符号"));
        check(more.get(3).get(1).text().equals("符号")
            && more.get(3).get(1).description().equals("切换符号键盘")
            && more.get(3).get(1).kind() == KeyboardLayout.LayerKeyKind.SYMBOL_PANEL);
        check(more.get(3).get(2).text().equals("中标")
            && more.get(3).get(2).kind() == KeyboardLayout.LayerKeyKind.PUNCTUATION_TOGGLE);
        check(KeyboardLayout.moreSymbolLayer(false).get(3).get(0).text().equals("ABC"));

        List<List<String>> symbols = KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS);
        check(symbols.size() == 3);
        check(symbols.get(0).equals(List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")));
        check(symbols.get(1).contains("\""));
        check(symbols.get(2).equals(List.of("(", ")", "[", "]", "<", ">", "\\", "-", "_", "=")));
        // 藏文的符号页把 `=` 换成叠写用的 `+`，其他键和其他方案不变。
        check(KeyboardLayout.symbolRowKey("=", true).equals("+"));
        check(KeyboardLayout.symbolRowKey("=", false).equals("="));
        check(KeyboardLayout.symbolRowKey("_", true).equals("_"));

        check(KeyboardLayout.resolveTouchLayout(false, false, 0, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, true, 0, "nine_key")
            == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT);
        // 14 键看引擎的网格（`key_grid`），存着 `fourteen_key` 而网格没开（本地模式）时是 26 键。
        check(KeyboardLayout.resolveTouchLayout(false, false, true, 0, "fourteen_key")
            == KeyboardLayout.FOURTEEN_KEY_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 0, "fourteen_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
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
        check(KeyboardLayout.resolveTouchLayout(true, false, 6, "handwriting")
            == KeyboardLayout.ZHUYIN_LAYOUT);
        // 注音 9 键按存下的布局字符串判断（与日语九键相同），不看 view 的 nine_key 标志。
        check(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT == 7);
        check(KeyboardLayout.resolveTouchLayout(false, true, 6, "nine_key")
            == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 6, "nine_key")
            == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT);
        check(!KeyboardLayout.carriesLetterCase(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT));
        check(KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT)
            == KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS));
        check(KeyboardLayout.resolveTouchLayout(false, false, 5, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 7, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        // 藏文同样用 26 个 QWERTY 键并保留字母大小写（威利转写区分大小写）。
        check(KeyboardLayout.resolveTouchLayout(false, false, 8, "twenty_six_key")
            == KeyboardLayout.STANDARD_TOUCH_LAYOUT);
        // 笔画不论偏好是 26 键还是九键都画自己的笔画键盘，只让位给手写。
        check(KeyboardLayout.resolveTouchLayout(false, false, 9, "twenty_six_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, true, 9, "nine_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(false, false, 9, "nine_key")
            == KeyboardLayout.STROKE_LAYOUT);
        check(KeyboardLayout.resolveTouchLayout(true, false, 9, "handwriting")
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

    private static List<String> texts(List<KeyboardLayout.LayerKey> row) {
        List<String> result = new java.util.ArrayList<>();
        for (KeyboardLayout.LayerKey key : row) result.add(key.text());
        return result;
    }
}
