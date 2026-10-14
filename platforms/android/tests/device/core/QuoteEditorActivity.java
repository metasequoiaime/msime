package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
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
import android.widget.TextView;
import app.msime.android.WindowLayout;

/** 输入框外的合成艾特与引用只响应退格事件，用真实 InputConnection 检查空框行为。 */
public final class QuoteEditorActivity extends Activity {
    private EditText editor;
    private TextView status;
    private boolean mention = true;
    private boolean quote = true;
    private int deleteEvents;
    private boolean unavailableBefore;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        unavailableBefore = getIntent().getBooleanExtra("unavailable_before", false);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        status = new TextView(this);
        status.setContentDescription("msime-test-quote-status");
        updateStatus();
        layout.addView(status);
        editor = new QuoteEditText(this);
        editor.setInputType(InputType.TYPE_CLASS_TEXT);
        editor.setContentDescription("msime-test-quote-editor");
        editor.setOnFocusChangeListener((view, focused) -> { if (focused) requestIme(); });
        layout.addView(editor);
        setContentView(layout);
        editor.requestFocus();
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && editor != null) requestIme();
    }

    private void requestIme() {
        for (long delay : new long[]{250, 750, 1500}) {
            editor.postDelayed(() -> {
                if (editor.hasFocus() && editor.hasWindowFocus()) {
                    InputMethodManager manager = getSystemService(InputMethodManager.class);
                    if (manager != null) manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                }
            }, delay);
        }
    }

    private void updateStatus() {
        status.setText("mention=" + mention + ";quote=" + quote + ";del=" + deleteEvents);
    }

    private final class QuoteEditText extends EditText {
        QuoteEditText(Context context) { super(context); }

        @Override public boolean onKeyDown(int code, KeyEvent event) {
            if (code == KeyEvent.KEYCODE_DEL) {
                deleteEvents++;
                if (getText().length() == 0) {
                    if (mention) mention = false;
                    else quote = false;
                    updateStatus();
                    return true;
                }
                updateStatus();
            }
            return super.onKeyDown(code, event);
        }

        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            InputConnection connection = super.onCreateInputConnection(info);
            if (!unavailableBefore || connection == null) return connection;
            return new InputConnectionWrapper(connection, false) {
                @Override public CharSequence getTextBeforeCursor(int length, int flags) { return null; }
            };
        }
    }
}
