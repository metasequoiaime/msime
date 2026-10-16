package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.widget.EditText;
import android.widget.LinearLayout;

/** 隔离的合成编辑器允许拒绝一次撤回，验证备份不会因拒写丢失。 */
public final class QuickDeleteEditorActivity extends Activity {
    volatile boolean rejectWrites;
    volatile boolean unknownPosition;
    volatile int acceptedWrites;
    EditText editor;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        app.msime.android.WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | android.view.WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        app.msime.android.WindowLayout.fitSystemBars(layout);
        editor = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                InputConnection target = super.onCreateInputConnection(info);
                return target == null ? null : new InputConnectionWrapper(target, false) {
                    @Override public boolean commitText(CharSequence text, int position) {
                        if (rejectWrites) return false;
                        acceptedWrites++;
                        return super.commitText(text, position);
                    }
                    @Override public android.view.inputmethod.ExtractedText getExtractedText(
                            android.view.inputmethod.ExtractedTextRequest request, int flags) {
                        return unknownPosition ? null : super.getExtractedText(request, flags);
                    }
                };
            }
        };
        // 显式允许大段合成文本，避免系统单行编辑器的默认长度限制截断多段回归。
        editor.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        editor.setFilters(new android.text.InputFilter[0]);
        editor.setHint("Synthetic quick-delete editor");
        editor.setContentDescription("msime-test-plain");
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) requestKeyboard();
        });
        layout.addView(editor, new LinearLayout.LayoutParams(-1, 180));
        EditText other = new EditText(this);
        other.setInputType(InputType.TYPE_CLASS_TEXT);
        other.setContentDescription("msime-test-other");
        other.setHint("Synthetic other editor");
        layout.addView(other, new LinearLayout.LayoutParams(-1, 180));
        setContentView(layout);
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused) requestKeyboard();
    }

    private void requestKeyboard() {
        for (long delay : new long[] {250, 750}) editor.postDelayed(() -> {
            if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
            getSystemService(android.view.inputmethod.InputMethodManager.class)
                .showSoftInput(editor, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT);
            if (android.os.Build.VERSION.SDK_INT >= 30 && editor.getWindowInsetsController() != null)
                editor.getWindowInsetsController().show(android.view.WindowInsets.Type.ime());
        }, delay);
    }
}
