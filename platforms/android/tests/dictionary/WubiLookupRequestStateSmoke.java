import app.msime.android.WubiLookupRequestState;

/** 五笔查码输入变化与视图重建后，旧请求不能覆盖当前结果或永久锁住查码按钮。 */
public final class WubiLookupRequestStateSmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        WubiLookupRequestState<String> state = new WubiLookupRequestState<>();
        check(state.begin() == 0, "blank text cannot start a lookup");

        state.edit("你好");
        long first = state.begin();
        check(first != 0 && state.busy(), "the first lookup starts");
        state.edit("世界");
        check(!state.busy() && state.result() == null, "editing clears the old result and unlocks search");
        long second = state.begin();
        check(second != 0 && second != first, "new text can start a new lookup before the old one ends");
        check(state.complete(second, "世界的编码"), "the new answer is accepted");
        check(!state.complete(first, "你好的编码"), "the old answer is rejected");
        check("世界的编码".equals(state.result()), "the new answer stays visible");

        state.edit("输入法");
        long beforeRotation = state.begin();
        state.viewDestroyed();
        check(!state.busy(), "a discarded view cannot leave the lookup button disabled");
        check(!state.complete(beforeRotation, "过期编码"), "a discarded view cannot accept its old answer");
        long afterRotation = state.begin();
        check(afterRotation != 0 && state.complete(afterRotation, "新的编码"), "the rebuilt view can retry");
        state.viewDestroyed();
        check("新的编码".equals(state.result()), "a completed answer survives a view rebuild");
        System.out.println("Android Wubi lookup request state passed");
    }
}
