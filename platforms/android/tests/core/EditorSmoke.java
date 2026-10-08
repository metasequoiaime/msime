import app.msime.android.EditorBridge;
import app.msime.android.EditorPolicy;
import app.msime.android.EnglishCapitalizationPolicy;
import android.text.InputType;
import java.util.ArrayList;
import java.util.List;

public final class EditorSmoke {
    static final class Sink implements EditorBridge.Sink {
        final List<String> calls = new ArrayList<>();
        boolean reject;
        public void begin() { calls.add("begin"); }
        public boolean commit(String value) { calls.add("commit:" + value); return !reject; }
        public boolean compose(String value) { calls.add("compose:" + value); return !reject; }
        public boolean finish() { calls.add("finish"); return true; }
        public boolean select(int start, int end) { calls.add("select:" + start + "," + end); return true; }
        public void end() { calls.add("end"); }
    }
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    public static void main(String[] args) {
        EditorBridge bridge = new EditorBridge();
        Sink sink = new Sink();
        check(bridge.apply(sink, null, "nihao"));
        sink.calls.clear();
        check(bridge.apply(sink, "你", "hao"));
        check(sink.calls.equals(List.of("begin", "commit:你", "compose:hao", "end")));
        sink.calls.clear();
        check(bridge.apply(sink, null, ""));
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "end")));
        sink.calls.clear();
        bridge.abandon(sink);
        check(sink.calls.equals(List.of("finish")));
        sink.calls.clear();
        // A Stroke composition is glyphs, not text: a tap elsewhere removes the marked 一丨 (offsets 4..6) and puts the tapped caret back, shifted when it lies after the region.
        check(bridge.apply(sink, null, "一丨"));
        sink.calls.clear();
        bridge.discard(sink, 4, 6, 10, 10);
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "select:8,8", "end")));
        sink.calls.clear();
        bridge.discard(sink, 4, 6, 1, 3);
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "select:1,3", "end")));
        sink.calls.clear();
        // A selection reaching into the removed region keeps what survives of it.
        bridge.discard(sink, 4, 6, 5, 9);
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "select:4,7", "end")));
        sink.calls.clear();
        // Without a reported region, or when the session stops, the region still goes and nothing is selected.
        bridge.discard(sink, -1, -1, 10, 10);
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "end")));
        sink.calls.clear();
        bridge.discard(sink);
        check(sink.calls.equals(List.of("begin", "compose:", "finish", "end")));
        sink.calls.clear();
        // After a discard the bridge composes nothing, so an empty update does not touch the editor again.
        check(bridge.apply(sink, null, ""));
        check(sink.calls.equals(List.of("begin", "end")));
        sink.calls.clear();
        sink.reject = true;
        check(!bridge.apply(sink, "🌲", "next"));
        check(sink.calls.equals(List.of("begin", "commit:🌲", "end")));
        check(EditorPolicy.useEngine(1));
        // No-suggestion text, and Chrome's address bar (URI + no suggestions), still compose.
        check(EditorPolicy.useEngine(0x80001));
        check(EditorPolicy.useEngine(0x80011));
        for (int type : new int[] {0, 2, 3, 0x81, 0x91, 0xe1}) check(!EditorPolicy.useEngine(type));
        check(EditorPolicy.allowLearning(0));
        check(!EditorPolicy.allowLearning(0x1000000));
        // Key heatmap exclusion: text and numeric passwords, and fields that ask for no learning.
        for (int type : new int[] {0x81, 0x91, 0xe1, 0x12}) check(EditorPolicy.excludesKeyStatistics(type, 0));
        check(!EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_TEXT, 0));
        check(!EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_NUMBER, 0));
        check(EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_TEXT, 0x1000000));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS));
        // 只要求不给建议的聊天框、搜索框按用户的默认中英文进框（#5998）；Chrome 地址栏仍因 URI 进英文。
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS));
        check(EditorPolicy.prefersLatin(0x80011));
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT));
        check(EditorPolicy.capitalizationMode(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS)
            == EnglishCapitalizationPolicy.Mode.ALL_CHARACTERS);
        check(EditorPolicy.capitalizationMode(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_CAP_WORDS)
            == EnglishCapitalizationPolicy.Mode.WORDS);
        check(EditorPolicy.capitalizationMode(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES)
            == EnglishCapitalizationPolicy.Mode.SENTENCES);
        check(EditorPolicy.capitalizationMode(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_URI | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES)
            == EnglishCapitalizationPolicy.Mode.NONE);
        System.out.println("Android editor contract: composition order, failure, external selection, discarded glyph composition and sensitive editor policy passed");
    }
}
