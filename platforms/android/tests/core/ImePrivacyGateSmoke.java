package app.msime.android;

import android.text.InputType;
import android.view.inputmethod.EditorInfo;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

/** 锁住 ImePrivacyGate 的口径（P17 / P19）：原有的按键计数判断不变；隐私模式、不做个性化学习、密码框三种情况下每一类记录都不做；本地输入日志只能由事件枚举构造。 */
public final class ImePrivacyGateSmoke {
    public static void main(String[] arguments) throws Exception {
        int text = InputType.TYPE_CLASS_TEXT;
        int password = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
        int webPassword = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
        int numberPassword = InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD;
        int noLearning = EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
        check(!ImePrivacyGate.excludesKeyStatistics(text, 0), "a plain text field counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(password, 0), "a password field never counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(numberPassword, 0), "a numeric password never counts keys");
        check(ImePrivacyGate.excludesKeyStatistics(text, noLearning), "no personalised learning means no counting");
        check(ImePrivacyGate.excludesKeyStatistics(null), "no editor means no counting");
        for (int type : new int[] {text, password, numberPassword}) {
            for (int options : new int[] {0, noLearning}) {
                check(ImePrivacyGate.excludesKeyStatistics(type, options)
                    == EditorPolicy.excludesKeyStatistics(type, options), "same answer as EditorPolicy");
            }
        }
        check(ImePrivacyGate.countsKeys(true, false), "switch on and editor allowed counts");
        check(!ImePrivacyGate.countsKeys(false, false), "switch off never counts");
        check(!ImePrivacyGate.countsKeys(true, true), "an excluded editor never counts");
        check(ImePrivacyGate.recordsTyping("/data/state", "你好"), "a commit with a directory is recorded");
        check(!ImePrivacyGate.recordsTyping("", "你好"), "no directory, nothing recorded");
        check(!ImePrivacyGate.recordsTyping("/data/state", ""), "an empty commit is not recorded");
        check(!ImePrivacyGate.recordsTyping("/data/state", null), "a missing commit is not recorded");
        check(ImePrivacyGate.capturesClipboard(true, true), "history on and store ready captures");
        check(!ImePrivacyGate.capturesClipboard(false, true), "history off never captures");
        check(!ImePrivacyGate.capturesClipboard(true, false), "no store, nothing captured");

        // 三种隐私情况 × 每一类记录：一律不做；普通输入框、隐私模式关着时一律允许。
        check(ImePrivacyGate.Record.values().length == 8, "every guarded record is listed");
        for (ImePrivacyGate.Record record : ImePrivacyGate.Record.values()) {
            check(ImePrivacyGate.allows(record, false, text, 0), record + " is allowed in a plain field");
            check(!ImePrivacyGate.allows(record, true, text, 0), record + " is suppressed in incognito");
            check(!ImePrivacyGate.allows(record, false, text, noLearning),
                record + " is suppressed when the field asks for no personalised learning");
            check(!ImePrivacyGate.allows(record, false, password, 0), record + " is suppressed in a password field");
            check(!ImePrivacyGate.allows(record, false, webPassword, 0), record + " is suppressed in a web password field");
            check(!ImePrivacyGate.allows(record, false, numberPassword, 0),
                record + " is suppressed in a numeric password field");
        }
        check(!ImePrivacyGate.allows(null, false, text, 0), "an unnamed record is never allowed");

        // 本地输入日志：行只由枚举、时间戳和耗时构造，写入方法不接受任何文本参数。
        check(ImeDebugOverlay.EventLog.line(ImeDebugOverlay.Event.KEY_DOWN, 12)
            .equals("{\"t_ms\":12,\"kind\":\"key_down\"}\n"), "an input event carries only time and kind");
        check(ImeDebugOverlay.EventLog.line(ImeDebugOverlay.Event.CANDIDATE_SHOWN, 5, 7)
            .equals("{\"t_ms\":5,\"kind\":\"candidate_shown\",\"duration_ms\":7}\n"), "a perf row carries only time, kind and duration");
        String[] kinds = {"key_down", "key_up", "candidate_shown", "candidate_selected", "commit", "backspace",
            "panel_open", "panel_close", "ime_start", "ime_finish"};
        check(ImeDebugOverlay.Event.values().length == kinds.length, "the P19 event kinds, no more");
        for (int index = 0; index < kinds.length; index++) {
            check(ImeDebugOverlay.Event.values()[index].wire().equals(kinds[index]), "event kind " + kinds[index]);
        }
        for (Class<?> type : new Class<?>[] {ImeDebugOverlay.class, ImeDebugOverlay.EventLog.class}) {
            for (Method method : type.getDeclaredMethods()) {
                // 写入入口都不是 private；private 的 submit / append 只接收由 line() 拼好的行。
                if (Modifier.isPrivate(method.getModifiers())) continue;
                String name = method.getName();
                if (!java.util.Set.of("event", "perf", "recordEvent", "recordPerf", "onKeyPressed", "line")
                        .contains(name)) continue;
                for (Class<?> parameter : method.getParameterTypes()) {
                    check(parameter != String.class && !CharSequence.class.isAssignableFrom(parameter)
                        && parameter != char[].class && parameter != byte[].class && parameter != Object.class,
                        name + " must not accept text");
                }
            }
            for (Constructor<?> constructor : type.getDeclaredConstructors()) {
                for (Class<?> parameter : constructor.getParameterTypes()) {
                    check(parameter != String.class && !CharSequence.class.isAssignableFrom(parameter),
                        "the log is not built from text");
                }
            }
        }
        // 环形上限：超出时丢最旧的整行，留到四分之三再续写。
        byte[] existing = "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        check(ImeDebugOverlay.EventLog.trimmed(existing, 4, 1024) == existing, "under the cap nothing is dropped");
        byte[] kept = ImeDebugOverlay.EventLog.trimmed(existing, 8, 24);
        String keptText = new String(kept, java.nio.charset.StandardCharsets.UTF_8);
        check(kept.length + 8 <= 24 && keptText.endsWith("{\"c\":3}\n") && !keptText.contains("{\"a\""),
            "over the cap the oldest whole lines go");
        check(ImeDebugOverlay.EventLog.MAX_BYTES == 1024 * 1024, "each log is bounded at 1 MiB");

        // 超限滚动也不能把固定 staging 路径的硬链接写到诊断目录之外。
        java.nio.file.Path logRoot = java.nio.file.Files.createTempDirectory("msime-event-log-");
        java.nio.file.Path external = java.nio.file.Files.createTempFile("msime-event-log-outside-", ".jsonl");
        byte[] sentinel = "outside-sentinel".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        java.nio.file.Files.write(external, sentinel);
        java.nio.file.Files.write(logRoot.resolve(ImeDebugOverlay.EventLog.EVENTS_FILE),
            new byte[ImeDebugOverlay.EventLog.MAX_BYTES]);
        java.nio.file.Files.createLink(logRoot.resolve("." + ImeDebugOverlay.EventLog.EVENTS_FILE + ".staging"), external);
        ImeDebugOverlay.EventLog log = new ImeDebugOverlay.EventLog(logRoot.toFile());
        Method append = ImeDebugOverlay.EventLog.class.getDeclaredMethod(
            "append", String.class, byte[].class);
        append.setAccessible(true);
        append.invoke(log, ImeDebugOverlay.EventLog.EVENTS_FILE,
            "synthetic\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        check(java.util.Arrays.equals(sentinel, java.nio.file.Files.readAllBytes(external)),
            "diagnostic log staging does not modify a hard-linked file");
        log.shutdown();
        System.out.println("Android IME privacy gate passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
