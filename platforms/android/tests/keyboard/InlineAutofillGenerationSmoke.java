package app.msime.android;

/** 填充服务的迟到视图不能越过输入框、响应或隐藏边界。 */
public final class InlineAutofillGenerationSmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
    public static void main(String[] args) {
        InlineAutofillGeneration state = new InlineAutofillGeneration();
        check(!state.accepts(state.nextResponse()), "response before input must be ignored");
        state.startInput();
        long first = state.nextResponse();
        check(state.accepts(first), "current response must be accepted");
        long replacement = state.nextResponse();
        check(!state.accepts(first) && state.accepts(replacement), "replacement invalidates pending old views");
        state.startInput();
        check(!state.accepts(replacement), "new editor invalidates previous views");
        long pending = state.nextResponse();
        state.finishInput();
        check(!state.accepts(pending), "hidden or finished input invalidates pending views");
        state.showInput();
        check(!state.accepts(pending), "showing input cannot revive pending views");
        check(state.accepts(state.nextResponse()), "fresh response after showing input works");
        System.out.println("Inline autofill generations: editor, response and visibility boundaries passed");
    }
}
