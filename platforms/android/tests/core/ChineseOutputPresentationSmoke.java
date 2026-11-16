import app.msime.android.CandidateGlossPolicy;
import app.msime.android.ChineseOutputPolicy;
import app.msime.android.CompositionCaretPolicy;
import app.msime.android.EditorBridge;
import app.msime.android.KoreanInputPolicy;
import app.msime.android.PhrasePreeditPolicy;

public final class ChineseOutputPresentationSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static String output(String text, boolean traditional, String variant, int scheme,
                                 String localMode, boolean dedicatedEnglish) {
        return ChineseOutputPolicy.output(text, traditional,
            ChineseOutputPolicy.applies(dedicatedEnglish, scheme, localMode),
            value -> fixtureConversion(value, ChineseOutputPolicy.traditionalVariantCode(variant)));
    }

    private static String fixtureConversion(String text, int variant) {
        check(text.equals("海滩吃里面软件") || text.equals("广东省") || text.equals("wg"),
            "only a prefix or engine annotation reaches conversion: " + text);
        return switch (text) {
            case "广东省" -> "廣東省";
            case "wg" -> "wg";
            default -> switch (variant) {
                case 0 -> "海灘喫裏面軟件";
                case 1 -> "海灘吃裡面軟件";
                case 2 -> "海灘吃裡面軟體";
                default -> throw new AssertionError("unexpected variant: " + variant);
            };
        };
    }

    private static final class Editor implements EditorBridge.Sink {
        String text = "";
        boolean marked;
        int selectionStart;
        int selectionEnd;

        public void begin() {}
        public void end() {}
        public boolean commit(String value) {
            text = value;
            return true;
        }
        public boolean compose(String value) {
            text = value;
            marked = true;
            return true;
        }
        public boolean finish() {
            marked = false;
            return true;
        }
        public boolean select(int start, int end) {
            selectionStart = start;
            selectionEnd = end;
            return true;
        }
    }

    private static void prefixAndSettle(boolean traditional, String variant, String expected) {
        String prefix = output("海滩吃里面软件", traditional, variant, 0, "none", false);
        check(prefix.equals(expected), "prefix follows the current output choice");
        String drawn = KoreanInputPolicy.composing(false, prefix, "paobu", "paobu");
        String settled = KoreanInputPolicy.composing(false, prefix, "paobu", "paobu");
        check(drawn.equals(expected + "paobu") && drawn.equals(settled),
            "drawn and settled strings have the same converted prefix and raw spelling");
        Editor editor = new Editor();
        EditorBridge bridge = new EditorBridge();
        check(bridge.apply(editor, null, drawn), "editor accepts the converted composition");
        bridge.abandon(editor);
        check(editor.text.equals(expected + "paobu") && !editor.marked,
            "cursor-move finishing retains converted text, not the raw prefix");

        String shuangpinDrawn = PhrasePreeditPolicy.composing(prefix, "pao'bu");
        String shuangpinSettled = PhrasePreeditPolicy.composing(prefix, "pcbu");
        check(bridge.apply(editor, null, shuangpinDrawn), "editor accepts drawn spelling");
        bridge.settle(editor, shuangpinSettled, 0, shuangpinDrawn.length(), 0, 0);
        check(editor.text.equals(expected + "pcbu") && !editor.marked
                && editor.selectionStart == 0 && editor.selectionEnd == 0,
            "different drawn/settled spelling also retains converted prefix and selection");

        int mark = 3;
        String title = PhrasePreeditPolicy.title(prefix,
            CompositionCaretPolicy.withMark("pao'bu", mark), false);
        int titleMark = CompositionCaretPolicy.markInTitle(prefix, mark, false);
        check(title.startsWith(expected) && title.charAt(titleMark) == CompositionCaretPolicy.CARET_MARK,
            "title and caret use the displayed prefix");
        check(PhrasePreeditPolicy.title(prefix, "Emoji", true).equals("Emoji"),
            "local-mode title is never prefixed");
        check(KoreanInputPolicy.composing(true, prefix, "gks", "한").equals("한"),
            "Korean composing reading remains intact");
    }

    private static void annotations(boolean traditional, String variant, String expected) {
        String annotation = output("广东省", traditional, variant, 0, "none", false);
        check(CandidateGlossPolicy.annotation(annotation, "Guangdong", true)
                .equals(expected + " · Guangdong"),
            "display converts the engine hint, not its translation");
        check(CandidateGlossPolicy.accessibilitySuffix(annotation, "Guangdong", true)
                .equals("，提示：" + expected + "，英文释义：Guangdong"),
            "accessibility reads the same converted hint");
        check(CandidateGlossPolicy.annotation(annotation, "unused", false).equals(expected),
            "hint conversion does not depend on gloss visibility");
        check(output("wg", traditional, variant, 2, "none", false).equals("wg"),
            "Latin wubi hint remains intact");
        for (int scheme : new int[] {3, 4}) {
            String original = scheme == 3 ? "かんじ" : "나라 이름 한";
            check(output(original, traditional, variant, scheme, "none", false).equals(original),
                "Japanese and Korean annotations bypass the converter");
        }
        check(output("广东省", traditional, variant, 0, "none", true).equals("广东省"),
            "dedicated English bypasses conversion");
        check(output("广东省", traditional, variant, 0, "temporary_japanese", false).equals("广东省"),
            "temporary Japanese bypasses conversion");
    }

    public static void main(String[] args) {
        String[] prefixes = {"海灘喫裏面軟件", "海灘吃裡面軟件", "海灘吃裡面軟體"};
        String[] variants = {"s2t", "s2tw", "s2twp"};
        for (int index = 0; index < variants.length; index++) {
            String variant = variants[index];
            prefixAndSettle(true, variant, prefixes[index]);
            annotations(true, variant, "廣東省");
            prefixAndSettle(false, variant, "海滩吃里面软件");
            annotations(false, variant, "广东省");
        }
        String expandedPrefix = ChineseOutputPolicy.output("软件", true, true,
            text -> "longer-converted-prefix");
        String title = PhrasePreeditPolicy.title(expandedPrefix, "pao|bu", false);
        check(title.charAt(CompositionCaretPolicy.markInTitle(expandedPrefix, 3, false))
                == CompositionCaretPolicy.CARET_MARK,
            "length-changing conversion uses displayed prefix length for the caret");
        System.out.println("Android Chinese output presentation: fixture converter routing, variant reload, "
            + "simplified restore, prefix display/settle and annotation accessibility passed");
    }
}
