package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import app.msime.android.WindowLayout;

/** 使用系统原生编辑历史的合成编辑器，不自行实现撤销栈。 */
public final class HistoryEditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(column);
        add(column, new EditText(this), "history-first", false);
        add(column, new EditText(this), "history-second", false);
        EditText unsupported = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                InputConnection target = super.onCreateInputConnection(info);
                return target == null ? null : new InputConnectionWrapper(target, false) {
                    @Override public boolean sendKeyEvent(KeyEvent event) {
                        // 模拟不实现编辑快捷键的自绘控件；不能回退成输入普通字母 z。
                        if (event.getKeyCode() == KeyEvent.KEYCODE_Z && event.isCtrlPressed()) return false;
                        return super.sendKeyEvent(event);
                    }
                };
            }
        };
        add(column, unsupported, "history-unsupported", false);
        add(column, new EditText(this), "history-password", true);
        setContentView(column);
    }

    private void add(LinearLayout column, EditText editor, String id, boolean password) {
        editor.setInputType(InputType.TYPE_CLASS_TEXT | (password
            ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_FLAG_MULTI_LINE));
        editor.setContentDescription(id);
        editor.setHint(id);
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) editor.postDelayed(() -> getSystemService(InputMethodManager.class)
                .showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT), 300);
        });
        column.addView(editor);
    }
}
