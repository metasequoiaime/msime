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
        sink.reject = true;
        check(!bridge.apply(sink, "🌲", "next"));
        check(sink.calls.equals(List.of("begin", "commit:🌲", "end")));
        check(EditorPolicy.useEngine(1));
        for (int type : new int[] {0, 2, 3, 0x81, 0x91, 0xe1, 0x80001}) check(!EditorPolicy.useEngine(type));
        check(EditorPolicy.allowLearning(0));
        check(!EditorPolicy.allowLearning(0x1000000));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS));
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
        System.out.println("Android editor contract: composition order, failure, external selection and sensitive editor policy passed");
    }
}
