package app.msime.android.test;

import app.msime.android.WindowLayout;
import android.app.Activity;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

/** 不区分组字的合成搜索框：每次文字改变都计数，绝不联网或收集真实输入。 */
public final class CompositionSearchEditorActivity extends Activity {
    private int changes;
    private int compositions;
    private TextView counts;

    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(root);
        counts = new TextView(this);
        counts.setContentDescription("composition-search-counts");
        EditText editor = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                InputConnection target = super.onCreateInputConnection(info);
                return target == null ? null : new InputConnectionWrapper(target, false) {
                    @Override public boolean setComposingText(CharSequence text, int position) {
                        compositions++;
                        updateCounts();
                        return super.setComposingText(text, position);
                    }
                };
            }
        };
        editor.setContentDescription("composition-search-editor");
        editor.setInputType(InputType.TYPE_CLASS_TEXT);
        editor.setSingleLine(true);
        editor.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {
                changes++;
                updateCounts();
            }
            @Override public void afterTextChanged(Editable text) { }
        });
        Button reset = new Button(this);
        reset.setText("重置合成搜索框");
        reset.setContentDescription("composition-search-reset");
        reset.setOnClickListener(view -> {
            editor.setText("前");
            editor.setSelection(editor.length());
            changes = 0;
            compositions = 0;
            updateCounts();
            editor.requestFocus();
            getSystemService(InputMethodManager.class).showSoftInput(editor, 0);
        });
        root.addView(counts);
        root.addView(reset);
        root.addView(editor);
        setContentView(root);
        reset.performClick();
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) editor.postDelayed(() ->
                getSystemService(InputMethodManager.class).showSoftInput(editor, 0), 300);
        });
        editor.postDelayed(() -> getSystemService(InputMethodManager.class).showSoftInput(editor, 0), 500);
    }

    private void updateCounts() { counts.setText(changes + ":" + compositions); }
}
