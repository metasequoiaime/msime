package app.msime.android;

/** 符号集插件异步读取只允许写回仍属于当前输入会话、且仍打开的面板。 */
public final class PluginSymbolLifecyclePolicySmoke {
    public static void main(String[] args) {
        check(PluginSymbolLifecyclePolicy.accepts(3, 3, true, "/prefs/a", "/prefs/a"),
            "the current open panel accepts its own plugin result");
        check(!PluginSymbolLifecyclePolicy.accepts(3, 4, true, "/prefs/a", "/prefs/a"),
            "a newer panel read invalidates an older plugin result");
        check(!PluginSymbolLifecyclePolicy.accepts(3, 3, false, "/prefs/a", "/prefs/a"),
            "a closed symbol panel rejects a late plugin result");
        check(!PluginSymbolLifecyclePolicy.accepts(3, 3, true, "/prefs/a", "/prefs/b"),
            "a result from an older preferences directory is rejected");
        check(!PluginSymbolLifecyclePolicy.accepts(3, 3, true, "", "/prefs/a"),
            "a result without a preferences directory is rejected");
        check(PluginSymbolLifecyclePolicy.advance(PluginSymbolLifecyclePolicy.MAX_GENERATION)
                == 0, "generation wraps without overflowing");
        System.out.println("Android plugin symbol lifecycle policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
