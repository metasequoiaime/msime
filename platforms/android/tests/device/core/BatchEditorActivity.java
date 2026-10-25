package app.msime.android.test;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityNodeInfo;
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
    private int imeRequestGeneration;
    private int imeSettledGeneration = -1;

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
        // 与 EditorActivity 同一约定：节点 extras 公开最后一轮显示请求是否已经发出，DeviceSmoke.tap 点完会等它再开始按键。
        editor.setAccessibilityDelegate(new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.getExtras().putBoolean("msime-test-ime-requests-settled",
                    imeSettledGeneration == imeRequestGeneration);
            }
        });
        editor.setOnFocusChangeListener((view, focused) -> { if (focused) requestIme(editor); });
        WindowLayout.fitSystemBars(editor);
        setContentView(editor);
        editor.requestFocus();
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && getCurrentFocus() instanceof EditText editor) requestIme(editor);
    }

    /**
     * 连续几轮请求显示输入法，与 EditorActivity.requestIme 相同。
     *
     * <p>原先只在 onCreate 里 post 一次。夜间构建 40164019651 的 API 28 上，前一个以产品为目标的套件结束时 :ime 被强停，这个用例从开始到超时输入法进程都没有再起来；改为与 EditorActivity 一样在获得焦点后连续请求，并让点按等最后一轮请求发出。
     */
    private void requestIme(EditText editor) {
        int generation = ++imeRequestGeneration;
        for (long delay : new long[] {250, 750, 1500, 3000}) {
            editor.postDelayed(() -> {
                if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
                InputMethodManager manager = getSystemService(InputMethodManager.class);
                if (manager != null) manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                if (Build.VERSION.SDK_INT >= 30) {
                    WindowInsetsController controller = editor.getWindowInsetsController();
                    if (controller != null) controller.show(WindowInsets.Type.ime());
                }
                if (delay == 3000 && generation == imeRequestGeneration) imeSettledGeneration = generation;
            }, delay);
        }
    }
}
