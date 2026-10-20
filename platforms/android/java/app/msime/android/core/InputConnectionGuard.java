package app.msime.android;

import android.os.Bundle;
import android.view.KeyEvent;
import android.view.inputmethod.InputContentInfo;
import android.view.inputmethod.InputConnection;

/** 编辑器可能在任意一次跨进程调用时断开；只捕获该边界的运行时异常，不读取或记录内容。 */
final class InputConnectionGuard {
    private InputConnectionGuard() {}

    /** 返回调用是否完成；编辑器返回 false 只代表不支持批量编辑，仍允许后续正常输入。 */
    static boolean beginBatchEdit(InputConnection connection) {
        if (connection == null) return false;
        try {
            connection.beginBatchEdit();
            return true;
        } catch (RuntimeException ignored) { return false; }
    }

    static boolean endBatchEdit(InputConnection connection) {
        if (connection == null) return false;
        try { return connection.endBatchEdit(); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean commitText(InputConnection connection, CharSequence text, int position) {
        if (connection == null) return false;
        try { return connection.commitText(text, position); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean deleteSurroundingText(InputConnection connection, int before, int after) {
        if (connection == null) return false;
        try { return connection.deleteSurroundingText(before, after); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean deleteSurroundingTextInCodePoints(InputConnection connection, int before, int after) {
        if (connection == null) return false;
        try { return connection.deleteSurroundingTextInCodePoints(before, after); }
        catch (RuntimeException ignored) { return false; }
    }

    static CharSequence textBeforeCursor(InputConnection connection, int length) {
        if (connection == null) return null;
        try { return connection.getTextBeforeCursor(length, 0); }
        catch (RuntimeException ignored) { return null; }
    }

    static CharSequence textAfterCursor(InputConnection connection, int length) {
        if (connection == null) return null;
        try { return connection.getTextAfterCursor(length, 0); }
        catch (RuntimeException ignored) { return null; }
    }

    static CharSequence selectedText(InputConnection connection) {
        if (connection == null) return null;
        try { return connection.getSelectedText(0); }
        catch (RuntimeException ignored) { return null; }
    }

    static boolean performEditorAction(InputConnection connection, int action) {
        if (connection == null) return false;
        try { return connection.performEditorAction(action); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean performContextMenuAction(InputConnection connection, int id) {
        if (connection == null) return false;
        try { return connection.performContextMenuAction(id); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean sendKeyEvent(InputConnection connection, KeyEvent event) {
        if (connection == null) return false;
        try { return connection.sendKeyEvent(event); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean requestCursorUpdates(InputConnection connection, int mode) {
        if (connection == null) return false;
        try { return connection.requestCursorUpdates(mode); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean commitContent(InputConnection connection, InputContentInfo content, int flags, Bundle options) {
        if (connection == null) return false;
        try { return connection.commitContent(content, flags, options); }
        catch (RuntimeException ignored) { return false; }
    }

    static boolean performPrivateCommand(InputConnection connection, String action, Bundle data) {
        if (connection == null) return false;
        try { return connection.performPrivateCommand(action, data); }
        catch (RuntimeException ignored) { return false; }
    }
}
