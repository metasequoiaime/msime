package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.view.View;
import android.view.WindowManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import app.msime.android.WindowLayout;

/** 只包含合成账号的系统自动填充编辑器，不接触设备上的真实凭据。 */
public final class InlineAutofillEditorActivity extends Activity {
    EditText username;
    EditText password;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setPadding(16, 80, 16, 16);
        WindowLayout.fitSystemBars(layout);
        username = editor(layout, "autofill-username", InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS, View.AUTOFILL_HINT_USERNAME);
        password = editor(layout, "autofill-password", InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_VARIATION_PASSWORD, View.AUTOFILL_HINT_PASSWORD);
        EditText plain = editor(layout, "autofill-plain", InputType.TYPE_CLASS_TEXT, null);
        plain.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);
        plain.requestFocus();
        setContentView(layout);
    }

    private EditText editor(LinearLayout layout, String description, int type, String hint) {
        EditText edit = new EditText(this);
        edit.setId(View.generateViewId());
        edit.setInputType(type);
        edit.setContentDescription(description);
        edit.setHint(description);
        if (hint != null) {
            edit.setAutofillHints(hint);
            edit.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
        }
        layout.addView(edit);
        return edit;
    }
}
