import app.msime.android.EditorBridge;
import app.msime.android.EditorPolicy;
import app.msime.android.CompositionTextPolicy;
import app.msime.android.EnglishCapitalizationPolicy;
import android.text.InputType;
import java.util.ArrayList;
import java.util.List;

public final class EditorSmoke {
    static class Sink implements EditorBridge.Sink {
        final List<String> calls = new ArrayList<>();
        boolean reject;
        public void begin() { calls.add("begin"); }
        public boolean commit(String value) { calls.add("commit:" + value); return !reject; }
        public boolean compose(String value) { calls.add("compose:" + value); return !reject; }
        public boolean finish() { calls.add("finish"); return true; }
        public boolean select(int start, int end) { calls.add("select:" + start + "," + end); return true; }
        public void end() { calls.add("end"); }
    }
    static final class VerificationSink extends Sink {
        boolean batch;
        String pending;
        String applied;
        boolean rejectVerification;
        boolean disconnectVerification;
        boolean disconnectEnd;
        @Override public void begin() { super.begin(); batch = true; }
        @Override public boolean compose(String text) { pending = text; return super.compose(text); }
        @Override public void end() {
            super.end();
            if (disconnectEnd) throw new IllegalStateException("synthetic disconnect");
            applied = pending;
            batch = false;
        }
        @Override public boolean verify() {
            calls.add("verify");
            check(!batch && "n".equals(applied));
            if (disconnectVerification) throw new IllegalStateException("synthetic disconnect");
            return !rejectVerification;
        }
    }
    static final class DisconnectedSink implements EditorBridge.Sink {
        public void begin() {}
        public boolean commit(String value) { throw new AssertionError("commit must not run"); }
        public boolean compose(String value) { throw new IllegalStateException("synthetic disconnected editor"); }
        public boolean finish() { throw new AssertionError("finish must not run"); }
        public boolean select(int start, int end) { throw new AssertionError("select must not run"); }
        public void end() { throw new IllegalStateException("synthetic disconnected editor"); }
    }
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    public static void main(String[] args) {
        // 读回验证必须在编辑器应用整批命令之后运行，拒写和断开仍由调用方统一处理。
        EditorBridge deferredBridge = new EditorBridge();
        VerificationSink deferred = new VerificationSink();
        check(deferredBridge.apply(deferred, null, "n"));
        check(deferred.calls.equals(List.of("begin", "compose:n", "end", "verify")));
        deferred.rejectVerification = true;
        check(!deferredBridge.apply(deferred, null, "n"));
        deferred.rejectVerification = false;
        deferred.disconnectVerification = true;
        check(!deferredBridge.apply(deferred, null, "n"));
        deferred.calls.clear();
        deferred.reject = true;
        check(!deferredBridge.apply(deferred, null, "n"));
        check(deferred.calls.equals(List.of("begin", "compose:n", "end")));
        deferred.calls.clear();
        deferred.reject = false;
        deferred.disconnectEnd = true;
        check(!deferredBridge.apply(deferred, null, "n"));
        check(deferred.calls.equals(List.of("begin", "compose:n", "end")));
        deferred.calls.clear();
        deferred.disconnectEnd = false;
        deferredBridge.settle(deferred, "n", 0, 1, 1, 1);
        check(deferred.calls.equals(List.of("begin", "compose:n", "finish", "select:1,1", "end")));
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
        // 日语画的是「かn」（4..6），确定时是「かん」：光标移走时留下确定的文字，点到的选区原样放回。
        check(bridge.apply(sink, null, "かn"));
        sink.calls.clear();
        bridge.settle(sink, "かん", 4, 6, 1, 3);
        check(sink.calls.equals(List.of("begin", "compose:かん", "finish", "select:1,3", "end")));
        sink.calls.clear();
        bridge.settle(sink, "かん", 4, 6, 10, 10);
        check(sink.calls.equals(List.of("begin", "compose:かん", "finish", "select:10,10", "end")));
        sink.calls.clear();
        // 新文字长短不同时，组字区之后的选区跟着平移，落在组字区里的不超出新文字。
        bridge.settle(sink, "x", 4, 6, 10, 10);
        check(sink.calls.equals(List.of("begin", "compose:x", "finish", "select:9,9", "end")));
        sink.calls.clear();
        bridge.settle(sink, "x", 4, 6, 6, 6);
        check(sink.calls.equals(List.of("begin", "compose:x", "finish", "select:5,5", "end")));
        sink.calls.clear();
        bridge.settle(sink, "abcd", 4, 6, 5, 8);
        check(sink.calls.equals(List.of("begin", "compose:abcd", "finish", "select:5,10", "end")));
        sink.calls.clear();
        // 编辑器报告已经没有组字区（发送后清空的聊天框、应用自己确定了组字）：setComposingText 会把「かん」插到光标处，所以不写任何文字，只结束组字。
        check(bridge.apply(sink, null, "かn"));
        sink.calls.clear();
        bridge.settle(sink, "かん", -1, -1, 0, 0);
        check(sink.calls.equals(List.of("finish")));
        sink.calls.clear();
        check(bridge.apply(sink, null, ""));
        check(sink.calls.equals(List.of("begin", "end")));
        // 长度过滤器可能把组字区截短；宿主读回光标前文字后应识别出这种情况，改为只在键盘上显示组字。
        check(CompositionTextPolicy.matchesEditorSuffix("合成文字nihao", "nihao"));
        check(!CompositionTextPolicy.matchesEditorSuffix("合成文字ni", "nihao"));
        check(CompositionTextPolicy.matchesEditorSuffix(null, "nihao"));
        check(CompositionTextPolicy.composingLength("合成文字ni", "nihao") == 2);
        sink.calls.clear();
        // 空的组字区同样算没有。
        check(bridge.apply(sink, null, "かn"));
        sink.calls.clear();
        bridge.settle(sink, "かん", 4, 4, 4, 4);
        check(sink.calls.equals(List.of("finish")));
        sink.calls.clear();
        check(bridge.apply(sink, null, ""));
        check(sink.calls.equals(List.of("begin", "end")));
        sink.calls.clear();
        // After a discard the bridge composes nothing, so an empty update does not touch the editor again.
        check(bridge.apply(sink, null, ""));
        check(sink.calls.equals(List.of("begin", "end")));
        sink.calls.clear();
        sink.reject = true;
        check(!bridge.apply(sink, "🌲", "next"));
        check(sink.calls.equals(List.of("begin", "commit:🌲", "end")));
        // 宿主可能在两次跨进程调用之间撤销 InputConnection；断开的编辑器应当只是本次编辑失败，不能让输入法进程崩溃。
        check(!bridge.apply(new DisconnectedSink(), null, "next"));
        check(EditorPolicy.useEngine(1));
        // No-suggestion text, and Chrome's address bar (URI + no suggestions), still compose.
        check(EditorPolicy.useEngine(0x80001));
        check(EditorPolicy.useEngine(0x80011));
        for (int type : new int[] {2, 3, 0x81, 0x91, 0xe1}) check(!EditorPolicy.useEngine(type));
        check(EditorPolicy.useEngine(0));
        check(EditorPolicy.numberInput(InputType.TYPE_CLASS_NUMBER));
        check(EditorPolicy.numberInput(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD));
        check(EditorPolicy.numberInput(InputType.TYPE_CLASS_PHONE));
        check(!EditorPolicy.numberInput(InputType.TYPE_CLASS_TEXT));
        check(EditorPolicy.allowLearning(0));
        check(!EditorPolicy.allowLearning(0x1000000));
        // Key heatmap exclusion: text and numeric passwords, and fields that ask for no learning.
        for (int type : new int[] {0x81, 0x91, 0xe1, 0x12}) check(EditorPolicy.excludesKeyStatistics(type, 0));
        check(!EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_TEXT, 0));
        check(!EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_NUMBER, 0));
        check(EditorPolicy.excludesKeyStatistics(InputType.TYPE_CLASS_TEXT, 0x1000000));
        // #9725：网址框（浏览器地址栏同时是搜索框）按默认中英文进框，不再临时切英文；英文模式下仍不给英文建议。
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.uri(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.uri(0x80011));
        check(!EditorPolicy.uri(InputType.TYPE_CLASS_TEXT));
        check(!EditorPolicy.uri(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS));
        // #10481：数字密码与文字密码一样进框临时切英文，「返回原键盘」是 26 键而不是上次记住的中文键面；普通数字框和电话框不切。
        check(EditorPolicy.prefersLatin(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD));
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_NUMBER));
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_PHONE));
        // #6147：邮箱后缀除了密码框都给，普通文本框、聊天框（不给建议、短消息）、搜索框、网址框和邮箱框都算。
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS));
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS));
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT));
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS));
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_SHORT_MESSAGE));
        check(EditorPolicy.offersEmailSuffixes(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI));
        check(EditorPolicy.offersEmailSuffixes(0x80011));
        check(EditorPolicy.offersEmailSuffixes(0));
        // 密码框（三种文字密码和数字密码）一律不给，也不读上文。
        for (int type : new int[] {0x81, 0x91, 0xe1, 0x12}) check(!EditorPolicy.offersEmailSuffixes(type));
        // 只要求不给建议的聊天框、搜索框按用户的默认中英文进框（#5998）；Chrome 地址栏（URI 加无建议）同样按默认中英文进框（#9725）。
        check(!EditorPolicy.prefersLatin(InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS));
        check(!EditorPolicy.prefersLatin(0x80011));
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
