import android.view.inputmethod.EditorInfo;
import app.msime.android.ReturnKeyAction;

public final class ReturnKeyActionSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static void title(int action, String expected) {
        check(ReturnKeyAction.title(action, false).equals(expected));
        check(ReturnKeyAction.performsEditorAction(action, false));
    }

    public static void main(String[] args) {
        check(ReturnKeyAction.title(EditorInfo.IME_ACTION_NONE, false).equals("换行"));
        check(ReturnKeyAction.title(EditorInfo.IME_ACTION_UNSPECIFIED, false).equals("换行"));
        check(!ReturnKeyAction.performsEditorAction(EditorInfo.IME_ACTION_NONE, false));
        check(!ReturnKeyAction.performsEditorAction(EditorInfo.IME_ACTION_UNSPECIFIED, false));
        title(EditorInfo.IME_ACTION_GO, "前往");
        title(EditorInfo.IME_ACTION_SEARCH, "搜索");
        title(EditorInfo.IME_ACTION_SEND, "发送");
        title(EditorInfo.IME_ACTION_NEXT, "下一项");
        title(EditorInfo.IME_ACTION_DONE, "完成");
        title(EditorInfo.IME_ACTION_PREVIOUS, "上一项");
        check(ReturnKeyAction.title(EditorInfo.IME_ACTION_SEND, true).equals("换行"));
        check(!ReturnKeyAction.performsEditorAction(EditorInfo.IME_ACTION_SEND, true));
        check(ReturnKeyAction.shouldPerformEditorAction(EditorInfo.IME_ACTION_SEND, false, false));
        check(!ReturnKeyAction.shouldPerformEditorAction(EditorInfo.IME_ACTION_SEND, false, true));
        check(!ReturnKeyAction.shouldPerformEditorAction(EditorInfo.IME_ACTION_SEND, true, false));
        check(ReturnKeyAction.title(255, false).equals("换行"));
        check(!ReturnKeyAction.performsEditorAction(255, false));
        System.out.println("Android return key: editor labels, dispatch and newline fallback passed");
    }
}
