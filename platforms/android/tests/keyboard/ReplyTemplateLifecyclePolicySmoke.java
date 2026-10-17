package app.msime.android;

/** 回复模板异步读取只允许写回仍在当前回复面板里的结果。 */
public final class ReplyTemplateLifecyclePolicySmoke {
    public static void main(String[] args) {
        check(ReplyTemplateLifecyclePolicy.accepts(4, 4, true, true),
            "the current open panel accepts its own template result");
        check(!ReplyTemplateLifecyclePolicy.accepts(4, 5, true, true),
            "a newer read invalidates an older template result");
        check(!ReplyTemplateLifecyclePolicy.accepts(4, 4, false, true),
            "a closed reply panel rejects a late template result");
        check(!ReplyTemplateLifecyclePolicy.accepts(4, 4, true, false),
            "a hidden reply panel rejects a late template result");
        check(ReplyTemplateLifecyclePolicy.advance(4) == 5,
            "closing a reply panel advances the template read generation");
        System.out.println("Android reply template lifecycle policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
