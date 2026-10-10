package app.msime.android.test;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.WindowInsetsController;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.content.Context;
import android.view.inputmethod.InputMethodManager;
import android.view.WindowManager;
import app.msime.android.WindowLayout;

/** Separate synthetic editor app: tests the system InputConnection, not a mock. */
public final class EditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        TextView title = new TextView(this);
        title.setText("MSIME synthetic editor fixture");
        layout.addView(title);
        EditText plain = new EditText(this);
        plain.setInputType(InputType.TYPE_CLASS_TEXT);
        plain.setContentDescription("msime-test-plain");
        plain.setHint("Plain editor");
        showImeOnFocus(plain);
        layout.addView(plain);
        EditText password = new EditText(this);
        password.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        password.setContentDescription("msime-test-password");
        password.setHint("Password editor");
        showImeOnFocus(password);
        layout.addView(password);
        EditText cursorAtEnd = new CursorAtEndEditText(this);
        cursorAtEnd.setInputType(InputType.TYPE_CLASS_TEXT);
        cursorAtEnd.setContentDescription("msime-test-cursor-at-end");
        cursorAtEnd.setHint("Editor that ignores newCursorPosition");
        showImeOnFocus(cursorAtEnd);
        layout.addView(cursorAtEnd);
        setContentView(layout);
    }

    /**
     * 不认 `commitText` 第二个参数的编辑器：不管输入法传多少，都当作 1，光标落在新文字后面。#6458 里 vivo「信息」和系统设置的搜索框就是这样，输入法用 `commitText(closing, 0)` 补上的后半个把光标带到了括号外面；这个输入框在模拟器上重现那种行为。
     */
    private static final class CursorAtEndEditText extends EditText {
        CursorAtEndEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitText(CharSequence text, int newCursorPosition) {
                    return super.commitText(text, 1);
                }
            };
        }
    }

    private void showImeOnFocus(EditText editor) {
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) requestIme(editor);
        });
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && getCurrentFocus() instanceof EditText editor) requestIme(editor);
    }

    private void requestIme(EditText editor) {
        for (long delay : new long[] {250, 750, 1500, 3000}) {
            editor.postDelayed(() -> {
                if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
                InputMethodManager manager = getSystemService(InputMethodManager.class);
                if (manager != null)
                    manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                if (Build.VERSION.SDK_INT >= 30) {
                    WindowInsetsController controller = editor.getWindowInsetsController();
                    if (controller != null) controller.show(WindowInsets.Type.ime());
                }
            }, delay);
        }
    }
}
