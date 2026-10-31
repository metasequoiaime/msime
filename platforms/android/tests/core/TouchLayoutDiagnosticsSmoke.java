package app.msime.android;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.HashSet;
import java.util.Set;

/** 锁住首帧布局诊断（#10389）的事件：偏好来源三选一，每个已知触屏布局在每个时机都有自己的种类，写出的行只有时间和种类，都只进本机诊断包。 */
public final class TouchLayoutDiagnosticsSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        // 建会话用的偏好来源：读到实时偏好、出厂副本、按记住的九键改过的出厂副本。
        check(TouchLayoutDiagnostics.sessionPreferences(true, false)
            == DiagnosticEventKind.ENGINE_SESSION_LIVE_PREFERENCES, "live preferences at creation");
        check(TouchLayoutDiagnostics.sessionPreferences(false, false)
            == DiagnosticEventKind.ENGINE_SESSION_FACTORY_PREFERENCES, "factory copy at creation");
        check(TouchLayoutDiagnostics.sessionPreferences(false, true)
            == DiagnosticEventKind.ENGINE_SESSION_HINTED_PREFERENCES, "hint-seeded copy at creation");
        // 读到实时偏好时副本不会被改，即使传进来也按实时偏好记。
        check(TouchLayoutDiagnostics.sessionPreferences(true, true)
            == DiagnosticEventKind.ENGINE_SESSION_LIVE_PREFERENCES, "live preferences win");

        // 报告者的那一下：会话到达前画 26 键、会话给出 26 键、实时偏好到达后画九键。
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.FIRST_DRAWN,
            KeyboardLayout.STANDARD_TOUCH_LAYOUT) == DiagnosticEventKind.TOUCH_LAYOUT_FIRST_DRAWN_TWENTY_SIX_KEY,
            "first drawn 26-key");
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.CREATED,
            KeyboardLayout.STANDARD_TOUCH_LAYOUT) == DiagnosticEventKind.TOUCH_LAYOUT_CREATED_TWENTY_SIX_KEY,
            "created 26-key");
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.LIVE,
            KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT) == DiagnosticEventKind.TOUCH_LAYOUT_LIVE_NINE_KEY,
            "live nine-key");
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.CREATED,
            KeyboardLayout.FOURTEEN_KEY_LAYOUT) == DiagnosticEventKind.TOUCH_LAYOUT_CREATED_FOURTEEN_KEY,
            "created 14-key");

        // KeyboardLayout 里每个触屏布局常量在每个时机都要有自己的种类：以后加了新布局却没登记，这里就失败，而不是在诊断包里悄悄缺一行。
        Set<String> wires = new HashSet<>();
        Set<Integer> layouts = new HashSet<>();
        for (Field field : KeyboardLayout.class.getDeclaredFields()) {
            int modifiers = field.getModifiers();
            if (field.getType() != int.class || !Modifier.isStatic(modifiers) || !Modifier.isFinal(modifiers)
                    || !field.getName().endsWith("_LAYOUT")) continue;
            int layout = field.getInt(null);
            check(layouts.add(layout), field.getName() + " is a distinct layout");
            String name = TouchLayoutDiagnostics.layoutName(layout);
            check(name != null && name.matches("[a-z_]+"), field.getName() + " has a fixed name");
            for (TouchLayoutDiagnostics.Stage stage : TouchLayoutDiagnostics.Stage.values()) {
                DiagnosticEventKind kind = TouchLayoutDiagnostics.layout(stage, layout);
                check(kind != null, field.getName() + " is recorded at " + stage);
                check(kind.wire().startsWith("touch_layout_") && kind.wire().endsWith("_" + name),
                    kind.wire() + " names the layout");
                check(!kind.uploadable(), kind.wire() + " stays in the local bundle");
                check(wires.add(kind.wire()), kind.wire() + " is distinct");
            }
        }
        check(layouts.size() >= 10, "the ten touch layouts are found by reflection");
        check(wires.size() == layouts.size() * TouchLayoutDiagnostics.Stage.values().length,
            "one kind per layout and stage");
        for (DiagnosticEventKind kind : new DiagnosticEventKind[] {DiagnosticEventKind.ENGINE_SESSION_LIVE_PREFERENCES,
                DiagnosticEventKind.ENGINE_SESSION_FACTORY_PREFERENCES, DiagnosticEventKind.ENGINE_SESSION_HINTED_PREFERENCES})
            check(!kind.uploadable(), kind.wire() + " stays in the local bundle");

        // 不认识的布局和缺了时机都不记：MSIMEInputService.recordTouchLayout 见到 null 就跳过，不会把 null 交给日志。
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.CREATED, -1) == null, "unknown layout");
        check(TouchLayoutDiagnostics.layout(TouchLayoutDiagnostics.Stage.CREATED, 99) == null, "future layout");
        check(TouchLayoutDiagnostics.layout(null, KeyboardLayout.STANDARD_TOUCH_LAYOUT) == null, "missing stage");

        // 写出的行只有时间和种类，诊断包的输入事件白名单按这个形状收。
        check(ImeDebugOverlay.EventLog.line(DiagnosticEventKind.TOUCH_LAYOUT_FIRST_DRAWN_TWENTY_SIX_KEY, 12)
            .equals("{\"t_ms\":12,\"kind\":\"touch_layout_first_drawn_twenty_six_key\"}\n"), "first drawn row");
        check(ImeDebugOverlay.EventLog.line(TouchLayoutDiagnostics.sessionPreferences(false, true), 7)
            .equals("{\"t_ms\":7,\"kind\":\"engine_session_hinted_preferences\"}\n"), "session preferences row");
        System.out.println("Android first-frame touch layout diagnostics passed");
    }
}
