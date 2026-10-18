package app.msime.android.test;

import android.app.Activity;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

/** 模拟输入一个字符就重启同一连接的登录页，只使用合成文本。 */
public final class RestartingEditorActivity extends Activity {
    private int connections;
    private TextView count;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        count = new TextView(this);
        count.setContentDescription("msime-restart-count");
        count.setText("0");
        layout.addView(count);
        EditText restarting = new EditText(this) {
            @Override public InputConnection onCreateInputConnection(EditorInfo info) {
                InputConnection connection = super.onCreateInputConnection(info);
                count.setText(Integer.toString(++connections));
                return connection;
            }
        };
        restarting.setId(101);
        restarting.setInputType(InputType.TYPE_CLASS_TEXT);
        restarting.setContentDescription("msime-restart-editor");
        restarting.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int length, int after) {}
            @Override public void onTextChanged(CharSequence text, int start, int before, int length) {}
            @Override public void afterTextChanged(Editable text) {
                restarting.post(() -> getSystemService(InputMethodManager.class).restartInput(restarting));
            }
        });
        layout.addView(restarting);
        EditText other = new EditText(this);
        other.setId(102);
        other.setInputType(InputType.TYPE_CLASS_TEXT);
        other.setContentDescription("msime-restart-other");
        layout.addView(other);
        setContentView(layout);
        restarting.requestFocus();
        restarting.post(() -> getSystemService(InputMethodManager.class)
            .showSoftInput(restarting, InputMethodManager.SHOW_IMPLICIT));
    }
}
