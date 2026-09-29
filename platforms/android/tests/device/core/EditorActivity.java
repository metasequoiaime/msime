package app.msime.android.test;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.view.inputmethod.InputMethodManager;
import android.view.WindowManager;
import app.msime.android.WindowLayout;

/** Separate synthetic editor app: tests the system InputConnection, not a mock. */
public final class EditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        TextView title = new TextView(this);
        title.setText("MSIME synthetic editor fixture");
        layout.addView(title);
        EditText plain = new EditText(this);
        plain.setInputType(InputType.TYPE_CLASS_TEXT);
        plain.setContentDescription("msime-test-plain");
        plain.setHint("Plain editor");
        showImeOnFocus(plain);
        layout.addView(plain);
        EditText password = new EditText(this);
        password.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        password.setContentDescription("msime-test-password");
        password.setHint("Password editor");
        showImeOnFocus(password);
        layout.addView(password);
        setContentView(layout);
    }

    private void showImeOnFocus(EditText editor) {
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) requestIme(editor);
        });
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && getCurrentFocus() instanceof EditText editor) requestIme(editor);
    }

    private void requestIme(EditText editor) {
        for (long delay : new long[] {250, 750, 1500, 3000}) {
            editor.postDelayed(() -> {
                if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
                InputMethodManager manager = getSystemService(InputMethodManager.class);
                if (manager != null)
                    manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                if (Build.VERSION.SDK_INT >= 30) {
                    WindowInsetsController controller = editor.getWindowInsetsController();
                    if (controller != null) controller.show(WindowInsets.Type.ime());
                }
            }, delay);
        }
    }
}
