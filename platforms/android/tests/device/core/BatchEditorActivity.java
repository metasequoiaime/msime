package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import app.msime.android.WindowLayout;
import java.util.ArrayList;
import java.util.List;

/** 模拟 Compose：批量写入结束前，读取上下文仍得到上一批的文字。 */
public final class BatchEditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        EditText editor = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                return new InputConnectionWrapper(super.onCreateInputConnection(info), false) {
                    private final List<Runnable> pending = new ArrayList<>();
                    private int depth;
                    @Override public boolean beginBatchEdit() {
                        depth++;
                        return super.beginBatchEdit();
                    }
                    @Override public boolean endBatchEdit() {
                        if (--depth == 0) {
                            List<Runnable> writes = new ArrayList<>(pending);
                            pending.clear();
                            for (Runnable write : writes) write.run();
                        }
                        return super.endBatchEdit();
                    }
                    @Override public boolean setComposingText(CharSequence text, int cursor) {
                        if (depth == 0) return super.setComposingText(text, cursor);
                        String copy = text.toString();
                        pending.add(() -> super.setComposingText(copy, cursor));
                        return true;
                    }
                    @Override public boolean commitText(CharSequence text, int cursor) {
                        if (depth == 0) return super.commitText(text, cursor);
                        String copy = text.toString();
                        pending.add(() -> super.commitText(copy, cursor));
                        return true;
                    }
                    @Override public boolean finishComposingText() {
                        if (depth == 0) return super.finishComposingText();
                        pending.add(() -> super.finishComposingText());
                        return true;
                    }
                    @Override public boolean deleteSurroundingText(int before, int after) {
                        if (depth == 0) return super.deleteSurroundingText(before, after);
                        pending.add(() -> super.deleteSurroundingText(before, after));
                        return true;
                    }
                };
            }
        };
        editor.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        editor.setContentDescription("msime-test-batch-editor");
        WindowLayout.fitSystemBars(editor);
        setContentView(editor);
        editor.requestFocus();
        editor.post(() -> getSystemService(InputMethodManager.class)
            .showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT));
    }
}
