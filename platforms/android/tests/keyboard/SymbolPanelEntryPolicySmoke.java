package app.msime.android;

import android.text.InputType;

/** 符号面板在不建会话的输入框里也能打开，密码框里按字面输入、不记「常用」（#10517）。 */
public final class SymbolPanelEntryPolicySmoke {
    private static final int TERMINAL = InputType.TYPE_CLASS_TEXT
        | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
    private static final int PASSWORD = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
    private static final int WEB_PASSWORD = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD;
    private static final int NUMBER_PASSWORD = InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD;
    private static final int PLAIN = InputType.TYPE_CLASS_TEXT;
    private static final int NO_SUGGESTIONS = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
    private static final int TYPE_NULL = InputType.TYPE_NULL;

    public static void main(String[] args) {
        // 能不能打开只看输入连接和面板，不看引擎会话：终端、密码框不建会话。
        check(SymbolPanelEntryPolicy.opens(true, true), "opens with a connection and a built panel");
        check(!SymbolPanelEntryPolicy.opens(false, true), "needs an input connection");
        check(!SymbolPanelEntryPolicy.opens(true, false), "needs the panel to be built");
        // 前提：这几种输入框确实不建会话，否则这次修的不是它们。
        for (int type : new int[] {TERMINAL, PASSWORD, WEB_PASSWORD, NUMBER_PASSWORD})
            check(!EditorPolicy.useEngine(type), "sessionless editor type " + type);
        check(EditorPolicy.useEngine(TYPE_NULL), "TYPE_NULL still builds a session");

        // 密码框里不补后半个，也不记常用。
        for (int type : new int[] {TERMINAL, PASSWORD, WEB_PASSWORD, NUMBER_PASSWORD}) {
            check(!SymbolPanelEntryPolicy.pairsClosingMark(type, true), "no closing half in password type " + type);
            check(!SymbolPanelEntryPolicy.recordsRecent(type, false), "no recents from password type " + type);
        }
        // 普通文本、关掉建议的文本和 TYPE_NULL 照旧。
        for (int type : new int[] {PLAIN, NO_SUGGESTIONS, TYPE_NULL}) {
            check(SymbolPanelEntryPolicy.pairsClosingMark(type, true), "closing half in type " + type);
            check(!SymbolPanelEntryPolicy.pairsClosingMark(type, false), "pairing switch off in type " + type);
            check(SymbolPanelEntryPolicy.recordsRecent(type, false), "recents in type " + type);
            check(!SymbolPanelEntryPolicy.recordsRecent(type, true), "suppressed learning in type " + type);
        }
        System.out.println("SymbolPanelEntryPolicySmoke: ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
