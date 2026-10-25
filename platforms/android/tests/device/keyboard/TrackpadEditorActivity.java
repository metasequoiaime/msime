package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.webkit.JavascriptInterface;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.WindowLayout;

/** 触控板的独立合成编辑器和按键接收宿主；不连接网络，不读取其他应用文档。 */
public final class TrackpadEditorActivity extends Activity {
    static final String TEXT = "alpha beta\ngamma delta\nepsilon zeta";
    static final String WRAPPED_TEXT = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    private TextView status;
    private int reads;
    private final StringBuilder keys = new StringBuilder();

    @Override public void onCreate(Bundle saved) {
        WindowLayout.theme(this);
        super.onCreate(saved);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(root);
        status = new TextView(this);
        status.setContentDescription("trackpad-status");
        status.setText("reads=0");
        root.addView(status);
        String kind = getIntent().getStringExtra("kind");
        if ("web".equals(kind)) {
            WebView web = new WebView(this);
            web.getSettings().setJavaScriptEnabled(true);
            web.addJavascriptInterface(new Object() {
                @JavascriptInterface public void selection(int start, int end) {
                    runOnUiThread(() -> status.setText("start=" + start + ";end=" + end));
                }
            }, "synthetic");
            web.setWebViewClient(new WebViewClient() {
                @Override public void onPageFinished(WebView view, String url) {
                    view.requestFocus();
                    view.evaluateJavascript("editor.focus();editor.setSelectionRange(8,8);report()", null);
                    showIme(view);
                }
            });
            root.addView(web, new LinearLayout.LayoutParams(-1, 200));
            web.loadDataWithBaseURL("https://synthetic.invalid/", "<!doctype html><meta charset=utf-8>"
                + "<textarea id=editor aria-label=trackpad-web style='width:95%;height:100px;font:20px monospace'>"
                + TEXT + "</textarea><script>function report(){synthetic.selection(editor.selectionStart,editor.selectionEnd)}"
                + "document.addEventListener('selectionchange',report);editor.addEventListener('keyup',report);"
                + "editor.addEventListener('input',report);</script>", "text/html", "utf-8", null);
        } else {
            boolean wrapped = "wrapped".equals(kind);
            boolean password = "password".equals(kind);
            boolean terminal = "terminal".equals(kind);
            boolean noAnchor = "no-anchor".equals(kind);
            boolean subscriptions = "subscriptions".equals(kind);
            StringBuilder updates = new StringBuilder("updates=");
            EditText editor = new EditText(this) {
                @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                    InputConnection base = super.onCreateInputConnection(info);
                    if (base == null) return null;
                    return new InputConnectionWrapper(base, false) {
                        @Override public boolean requestCursorUpdates(int mode) {
                            boolean accepted = !noAnchor && super.requestCursorUpdates(mode);
                            if (subscriptions) { updates.append(mode).append(';'); status.setText(updates.toString()); }
                            return accepted;
                        }
                        @Override public boolean requestCursorUpdates(int mode, int filter) {
                            boolean accepted = !noAnchor && super.requestCursorUpdates(mode, filter);
                            if (subscriptions) { updates.append(mode).append(';'); status.setText(updates.toString()); }
                            return accepted;
                        }
                        @Override public CharSequence getTextBeforeCursor(int length, int flags) {
                            if (password) { status.setText("reads=" + ++reads); return null; }
                            return terminal ? null : super.getTextBeforeCursor(length, flags);
                        }
                        @Override public CharSequence getTextAfterCursor(int length, int flags) {
                            if (password) { status.setText("reads=" + ++reads); return null; }
                            return terminal ? null : super.getTextAfterCursor(length, flags);
                        }
                        @Override public android.view.inputmethod.ExtractedText getExtractedText(
                                android.view.inputmethod.ExtractedTextRequest request, int flags) {
                            if (password) { status.setText("reads=" + ++reads); return null; }
                            return terminal ? null : super.getExtractedText(request, flags);
                        }
                        @Override public boolean sendKeyEvent(KeyEvent event) {
                            if (!terminal) return super.sendKeyEvent(event);
                            if (keys.length() > 0) keys.append(',');
                            keys.append(event.getAction()).append(':').append(event.getKeyCode())
                                .append(':').append(event.getMetaState());
                            status.setText(keys.toString());
                            return true;
                        }
                    };
                }
            };
            editor.setInputType(InputType.TYPE_CLASS_TEXT | (password
                ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_FLAG_MULTI_LINE));
            editor.setTypeface(android.graphics.Typeface.MONOSPACE);
            editor.setContentDescription("trackpad-editor");
            editor.setText(wrapped ? WRAPPED_TEXT : password ? "synthetic-private-password" : "long-selection".equals(kind) ? String.join("", java.util.Collections.nCopies(200, TEXT)) : TEXT);
            editor.setSelection(wrapped ? 14 : 8);
            root.addView(editor, new LinearLayout.LayoutParams(wrapped
                ? Math.round(160 * getResources().getDisplayMetrics().density) : -1, 180));
            if (wrapped) editor.addOnLayoutChangeListener((view, left, top, right, bottom,
                    oldLeft, oldTop, oldRight, oldBottom) -> {
                android.text.Layout layout = editor.getLayout();
                if (layout == null) return;
                int line = Math.min(1, layout.getLineCount() - 1);
                // 合成宿主报告真实排版边界；没有换行符的段落必须发生自动折行。
                status.setText("wrap-anchor=" + (layout.getLineStart(line) + 2)
                    + ";wrap-start=" + layout.getLineStart(line) + ";wrap-end="
                    + (layout.getLineEnd(line) - 1));
            });
            root.post(() -> { editor.requestFocus(); showIme(editor); });
        }
        setContentView(root);
    }

    private void showIme(View view) {
        view.post(() -> {
            InputMethodManager manager = getSystemService(InputMethodManager.class);
            if (manager != null) manager.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
        });
    }
}
