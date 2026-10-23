package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.widget.EditText;
import android.widget.LinearLayout;

/** 合成复制编辑器，可明确拒绝菜单动作或隐藏位置，验证输入法不会改走读取文本写剪贴板。 */
public final class SpaceCopyEditorActivity extends Activity {
    private int copies;
    private int selections;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        app.msime.android.WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        app.msime.android.WindowLayout.fitSystemBars(layout);
        if (getIntent().getBooleanExtra("webview", false)) {
            android.webkit.WebView web = new android.webkit.WebView(this);
            layout.addView(web, new LinearLayout.LayoutParams(-1, 400));
            setContentView(layout);
            web.loadDataWithBaseURL("https://synthetic.invalid/",
                "<html><head><meta name='viewport' content='width=device-width,initial-scale=1'></head>"
                + "<body><textarea aria-label='msime-test-copy-web' style='width:95%;height:100px;font-size:20px'>"
                + "合成复制😀甲乙\n第二行</textarea></body></html>", "text/html", "UTF-8", null);
            return;
        }
        EditText editor = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                InputConnection target = super.onCreateInputConnection(info);
                return target == null ? null : new InputConnectionWrapper(target, false) {
                    @Override public boolean performContextMenuAction(int action) {
                        if (action == android.R.id.copy) {
                            copies++;
                            if (getIntent().getBooleanExtra("reject-copy", false)) return false;
                        }
                        if (action == android.R.id.selectAll) {
                            selections++;
                            if (getIntent().getBooleanExtra("reject-all", false)) return false;
                        }
                        return super.performContextMenuAction(action);
                    }
                    @Override public android.view.inputmethod.ExtractedText getExtractedText(
                            android.view.inputmethod.ExtractedTextRequest request, int flags) {
                        return getIntent().getBooleanExtra("unknown-position", false)
                            ? null : super.getExtractedText(request, flags);
                    }
                };
            }

            @Override public void onInitializeAccessibilityNodeInfo(android.view.accessibility.AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(info);
                info.getExtras().putInt("synthetic-copy-attempts", copies);
                info.getExtras().putInt("synthetic-select-all-attempts", selections);
            }
        };
        editor.setInputType(InputType.TYPE_CLASS_TEXT | (getIntent().getBooleanExtra("password", false)
            ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_FLAG_MULTI_LINE));
        editor.setContentDescription("msime-test-copy-source");
        editor.setText("合成复制😀甲乙\n第二行");
        editor.setSelection(getIntent().getIntExtra("selection-start", 3),
            getIntent().getIntExtra("selection-end", 3));
        layout.addView(editor, new LinearLayout.LayoutParams(-1, 240));
        EditText other = new EditText(this);
        other.setContentDescription("msime-test-copy-other");
        other.setText("另一个合成输入框");
        layout.addView(other, new LinearLayout.LayoutParams(-1, 180));
        setContentView(layout);
        editor.requestFocus();
        editor.postDelayed(() -> getSystemService(android.view.inputmethod.InputMethodManager.class)
            .showSoftInput(editor, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT), 500);
    }
}
