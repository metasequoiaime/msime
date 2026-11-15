package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.os.Bundle;
import android.text.InputType;
import android.text.Editable;
import android.text.Spanned;
import android.text.TextWatcher;
import android.text.style.ReplacementSpan;
import android.graphics.Canvas;
import android.graphics.Paint;
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

/** 以真实 InputConnection 验证空框引用和隐藏样式的合成内联表情，删除规则由编辑器持有。 */
public final class QuoteEditorActivity extends Activity {
    private EditText editor;
    private TextView status;
    private boolean mention = true;
    private boolean quote = true;
    private int deleteEvents;
    private boolean unavailableBefore;
    private boolean privateInline;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        unavailableBefore = getIntent().getBooleanExtra("unavailable_before", false);
        privateInline = getIntent().getBooleanExtra("private_inline", false);
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
        if (privateInline) editor.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}
            @Override public void afterTextChanged(Editable text) {
                // 夹具只装饰合成标签；删除规则仍由系统 TextView 的真实按键处理实现。
                for (SyntheticInlineObject span : text.getSpans(0, text.length(), SyntheticInlineObject.class))
                    text.removeSpan(span);
                String value = text.toString();
                for (int start = value.indexOf("[合成]"); start >= 0; start = value.indexOf("[合成]", start + 4))
                    text.setSpan(new SyntheticInlineObject(), start, start + 4, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
                for (int index = 0; index < text.length(); index++) if (text.charAt(index) == '\ufffc')
                    text.setSpan(new SyntheticInlineObject(), index, index + 1, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            }
        });
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
            if ((!unavailableBefore && !privateInline) || connection == null) return connection;
            return new InputConnectionWrapper(connection, false) {
                @Override public CharSequence getTextBeforeCursor(int length, int flags) {
                    if (unavailableBefore) return null;
                    CharSequence before = super.getTextBeforeCursor(length, flags);
                    // 模拟编辑器的私有样式不对输入法公开，底层 Editable 仍保留替换范围。
                    return before == null ? null : before.toString();
                }
            };
        }
    }

    private static final class SyntheticInlineObject extends ReplacementSpan {
        @Override public int getSize(Paint paint, CharSequence text, int start, int end, Paint.FontMetricsInt metrics) { return 24; }
        @Override public void draw(Canvas canvas, CharSequence text, int start, int end, float x,
                                   int top, int y, int bottom, Paint paint) {
            canvas.drawText("●", x, y, paint);
        }
    }
}
