import app.msime.android.EditorBridge;
import app.msime.android.ChineseOutputCompositionSync;
import android.text.Spanned;

public final class ChineseOutputCompositionSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class Editor implements EditorBridge.Sink {
        String text = "前后文";
        int composingStart = -1;
        int composingEnd = -1;
        int selectionStart = 1;
        int selectionEnd = 1;
        int writes;

        public void begin() {}
        public void end() {}
        public boolean commit(String value) {
            replace(value);
            composingStart = composingEnd = -1;
            return true;
        }
        public boolean compose(String value) {
            int start = composingStart >= 0 ? composingStart : Math.min(selectionStart, selectionEnd);
            replace(value);
            composingStart = value.isEmpty() ? -1 : start;
            composingEnd = value.isEmpty() ? -1 : start + value.length();
            return true;
        }
        private void replace(String value) {
            int start = composingStart >= 0 ? composingStart : Math.min(selectionStart, selectionEnd);
            int end = composingStart >= 0 ? composingEnd : Math.max(selectionStart, selectionEnd);
            text = text.substring(0, start) + value + text.substring(end);
            selectionStart = selectionEnd = start + value.length();
            writes++;
        }
        public boolean finish() {
            composingStart = composingEnd = -1;
            return true;
        }
        public boolean select(int start, int end) {
            selectionStart = start;
            selectionEnd = end;
            return true;
        }
    }

    private static Editor composing(EditorBridge bridge, String oldText) {
        Editor editor = new Editor();
        check(bridge.apply(editor, null, oldText), "initial composing write succeeds");
        check(editor.text.equals("前" + oldText + "后文"), "editor holds pre-change glyphs");
        return editor;
    }

    private static void finish(EditorBridge bridge, Editor editor, String drawn, String settled) {
        String marked = ChineseOutputCompositionSync.markedText(
            new MarkedText(editor.text, editor.composingStart, editor.composingEnd),
            editor.composingStart, editor.composingEnd);
        bridge.finishInPlace(editor, drawn, settled, marked, editor.composingStart, editor.composingEnd,
            editor.selectionStart, editor.selectionEnd);
        check(editor.composingStart == -1, "composition is finished");
    }

    private static void changedThenCaretOut(String oldText, String newText) {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, oldText);
        editor.select(editor.text.length(), editor.text.length() - 1);
        finish(bridge, editor, newText, newText);
        check(editor.text.equals("前" + newText + "后文"),
            "equal new drawn/settled must replace old editor glyphs without further typing");
        check(editor.selectionStart == editor.text.length()
                && editor.selectionEnd == editor.text.length() - 1,
            "reversed user selection after the region follows replacement length");
    }

    private static void refreshedThenCaretOut(String oldText, String newText) {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, oldText);
        ChineseOutputCompositionSync.refreshMarked(bridge, editor, newText,
            new MarkedText(editor.text, editor.composingStart, editor.composingEnd),
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前" + newText + "后文"), "live composing follows the changed output");
        int writes = editor.writes;
        check(bridge.refreshComposition(editor, newText, newText,
                editor.composingStart, editor.composingEnd, editor.selectionStart, editor.selectionEnd),
            "same-choice preference reload succeeds");
        check(editor.writes == writes, "same-choice reload does not rewrite");
        editor.select(0, 0);
        finish(bridge, editor, newText, newText);
        check(editor.text.equals("前" + newText + "后文") && editor.writes == writes,
            "synced composition finishes without redundant replacement or typing");
        check(editor.selectionStart == 0 && editor.selectionEnd == 0, "selection before region stays put");
    }

    private record MarkedText(String text, int start, int end) implements Spanned {
        public int length() { return text.length(); }
        public char charAt(int index) { return text.charAt(index); }
        public CharSequence subSequence(int from, int to) { return text.subSequence(from, to); }
        public <T> T[] getSpans(int from, int to, Class<T> type) {
            @SuppressWarnings("unchecked")
            T[] spans = (T[]) java.lang.reflect.Array.newInstance(type, 1);
            spans[0] = type.cast(this);
            return spans;
        }
        public int getSpanStart(Object span) { return start; }
        public int getSpanEnd(Object span) { return end; }
        public int getSpanFlags(Object span) { return Spanned.SPAN_COMPOSING; }
        @SuppressWarnings("rawtypes")
        public int nextSpanTransition(int from, int limit, Class type) { return limit; }
    }

    private static void movedBeforeRefresh() {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, "海滩");
        editor.select(0, 1);
        bridge.refreshComposition(editor, "海灘", "海滩", editor.composingStart,
            editor.composingEnd, editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前海滩后文") && editor.selectionStart == 0 && editor.selectionEnd == 1,
            "refresh must not move an external selection");
        finish(bridge, editor, "海灘", "海灘");
        check(editor.text.equals("前海灘后文") && editor.selectionStart == 0 && editor.selectionEnd == 1,
            "finish repairs skipped refresh and restores external selection");
    }

    private static void editorRewrittenThenCaretOut() {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, "海滩");
        editor.text = "前别字后文";
        int writes = editor.writes;
        ChineseOutputCompositionSync.refreshMarked(bridge, editor, "海灘",
            new MarkedText(editor.text, editor.composingStart, editor.composingEnd),
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前别字后文") && editor.writes == writes,
            "refresh preserves the editor-rewritten marked text");
        editor.select(editor.text.length(), editor.text.length() - 1);
        finish(bridge, editor, "海灘", "海灘");
        check(editor.text.equals("前别字后文") && editor.writes == writes,
            "caret-out must not replace editor-rewritten 别字 with 海灘");
        check(editor.selectionStart == editor.text.length()
                && editor.selectionEnd == editor.text.length() - 1,
            "finish-only preserves the editor's reversed selection");
    }

    private static void lostMark(int start, int end) {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, "海滩");
        editor.composingStart = editor.composingEnd = -1;
        editor.select(0, editor.text.length());
        int writes = editor.writes;
        bridge.refreshComposition(editor, "海灘", "", start, end,
            editor.selectionStart, editor.selectionEnd);
        bridge.finishInPlace(editor, "海灘", "海灘", null, start, end,
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前海滩后文") && editor.writes == writes,
            "lost or empty mark never inserts at the caret or overwrites selection");
        check(editor.selectionStart == 0 && editor.selectionEnd == editor.text.length(),
            "lost-mark selection is untouched");
    }

    private static void unreadableMark() {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, "海滩");
        int start = editor.composingStart;
        int end = editor.composingEnd;
        check(ChineseOutputCompositionSync.markedText(editor.text, start, end) == null,
            "unstyled extraction cannot verify a composing mark");
        check(ChineseOutputCompositionSync.markedText(null, start, end) == null,
            "missing extraction cannot verify a composing mark");
        MarkedText marked = new MarkedText(editor.text, start, end);
        check(ChineseOutputCompositionSync.markedText(marked, start + 1, end) == null,
            "stale composing bounds cannot verify the reported region");
        check(ChineseOutputCompositionSync.markedText(marked, start, end + editor.text.length()) == null,
            "out-of-range composing bounds cannot be read");
        editor.select(0, 1);
        int writes = editor.writes;
        bridge.finishInPlace(editor, "海灘", "海灘", null, start, end,
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前海滩后文") && editor.writes == writes
                && editor.composingStart == -1 && editor.selectionStart == 0 && editor.selectionEnd == 1,
            "unreadable mark finishes without unverified replacement or selection changes");
        editor = composing(bridge, "n");
        bridge.finishInPlace(editor, "n", "ん", null, editor.composingStart, editor.composingEnd,
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前n后文"), "unreadable Japanese mark is not blindly replaced");
    }

    private static void nonChineseAndUnchanged() {
        EditorBridge bridge = new EditorBridge();
        Editor editor = composing(bridge, "n");
        editor.select(editor.text.length(), editor.text.length());
        finish(bridge, editor, "n", "ん");
        check(editor.text.equals("前ん后文"), "Japanese trailing n still settles");
        editor = composing(bridge, "海滩");
        int writes = editor.writes;
        finish(bridge, editor, "海滩", "海滩");
        check(editor.writes == writes && editor.text.equals("前海滩后文"),
            "unchanged simplified output still abandons without replacement");
        editor = composing(bridge, "一丨");
        editor.select(editor.text.length(), editor.text.length());
        bridge.discard(editor, editor.composingStart, editor.composingEnd,
            editor.selectionStart, editor.selectionEnd);
        check(editor.text.equals("前后文") && editor.selectionStart == 3,
            "stroke discard still removes the mark and shifts selection");
    }

    public static void main(String[] args) {
        editorRewrittenThenCaretOut();
        changedThenCaretOut("海滩", "海灘");
        changedThenCaretOut("海滩paobu", "海灘paobu");
        changedThenCaretOut("喫裏面軟件", "吃裡面軟件");
        changedThenCaretOut("吃裡面軟件", "吃裡面軟體");
        changedThenCaretOut("海灘", "海滩");
        changedThenCaretOut("廣東", "广东");
        changedThenCaretOut("甲乙", "甲乙丙");
        refreshedThenCaretOut("海滩paobu", "海灘paobu");
        refreshedThenCaretOut("喫裏面軟件", "吃裡面軟件");
        refreshedThenCaretOut("吃裡面軟件", "吃裡面軟體");
        refreshedThenCaretOut("海灘", "海滩");
        refreshedThenCaretOut("廣東", "广东");
        movedBeforeRefresh();
        lostMark(-1, -1);
        lostMark(1, 1);
        unreadableMark();
        nonChineseAndUnchanged();
        System.out.println("Chinese output mid-composition sync/finish smoke passed");
    }
}
