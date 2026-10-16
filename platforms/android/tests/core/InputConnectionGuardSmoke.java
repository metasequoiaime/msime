package app.msime.android;

import android.view.inputmethod.InputConnection;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;

/** 合成断开连接验证边界；批量编辑不支持时仍然允许输入。 */
public final class InputConnectionGuardSmoke {
    public static void main(String[] args) {
        InputConnection disconnected = (InputConnection) Proxy.newProxyInstance(
            InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
            (proxy, method, arguments) -> {
                throw new IllegalStateException("synthetic disconnected editor");
            });
        check(!InputConnectionGuard.beginBatchEdit(disconnected));
        check(!InputConnectionGuard.endBatchEdit(disconnected));
        check(!InputConnectionGuard.commitText(disconnected, "x", 1));
        check(!InputConnectionGuard.deleteSurroundingText(disconnected, 1, 0));
        check(!InputConnectionGuard.deleteSurroundingTextInCodePoints(disconnected, 1, 0));
        check(InputConnectionGuard.textBeforeCursor(disconnected, 2) == null);
        check(InputConnectionGuard.textAfterCursor(disconnected, 2) == null);
        check(InputConnectionGuard.selectedText(disconnected) == null);
        check(!InputConnectionGuard.performEditorAction(disconnected, 0));
        check(!InputConnectionGuard.performContextMenuAction(disconnected, 0));
        check(!InputConnectionGuard.sendKeyEvent(disconnected, null));
        check(!InputConnectionGuard.requestCursorUpdates(disconnected, 0));
        check(!InputConnectionGuard.commitContent(disconnected, null, 0, null));
        List<String> calls = new ArrayList<>();
        InputConnection unbatched = (InputConnection) Proxy.newProxyInstance(
            InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
            (proxy, method, arguments) -> { calls.add(method.getName()); return false; });
        check(InputConnectionGuard.beginBatchEdit(unbatched));
        check(!InputConnectionGuard.commitText(unbatched, "x", 1));
        check(!InputConnectionGuard.endBatchEdit(unbatched));
        check(calls.equals(List.of("beginBatchEdit", "commitText", "endBatchEdit")));
        System.out.println("InputConnection disconnect and unsupported batching guards passed");
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }
}
