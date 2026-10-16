package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import app.msime.android.WindowLayout;

/** 独立的合成编辑器，覆盖英文纠错与禁止改写的输入框。 */
public final class EnglishCorrectionEditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        add(layout, "correction-plain", InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        add(layout, "correction-no-suggestions", InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        add(layout, "correction-url", InputType.TYPE_TEXT_VARIATION_URI);
        add(layout, "correction-email", InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        add(layout, "correction-password", InputType.TYPE_TEXT_VARIATION_PASSWORD);
        setContentView(layout);
    }

    private void add(LinearLayout layout, String description, int traits) {
        EditText editor = new EditText(this);
        editor.setInputType(InputType.TYPE_CLASS_TEXT | traits);
        editor.setContentDescription(description);
        editor.setHint(description);
        editor.setSingleLine(true);
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) view.post(() -> getSystemService(InputMethodManager.class)
                .showSoftInput(view, InputMethodManager.SHOW_IMPLICIT));
        });
        layout.addView(editor);
    }
}
