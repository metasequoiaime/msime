package app.msime.android;

import android.view.inputmethod.InputConnection;

/** 光标位置不可读时的快速清除退路；这种路径不生成撤回备份。 */
final class QuickDeleteFallback {
    private QuickDeleteFallback() {}

    /**
     * 让编辑器自己按“光标前全部内容”解释删除范围。编辑器会把范围截到实际文本长度，
     * 因此不会删除光标后的内容；返回值只表示请求是否被编辑器接受。
     */
    static boolean deleteBeforeCursor(InputConnection connection) {
        return InputConnectionGuard.deleteSurroundingText(connection, Integer.MAX_VALUE, 0);
    }
}
