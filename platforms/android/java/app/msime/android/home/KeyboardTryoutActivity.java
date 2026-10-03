package app.msime.android.home;

import android.os.Bundle;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.View;
import android.view.inputmethod.InputMethodManager;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.appbar.MaterialToolbar;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.textfield.TextInputEditText;
import app.msime.android.R;
import app.msime.android.BackendAccount;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

/**
 * 试用键盘: an editor that exists only so the keyboard can be raised against it, matching the Apple
 * app's tryout screen.
 *
 * The keyboard here is the real input method bound to this editor, not the still preview on the
 * 键盘 tab. Nothing typed is read, stored or sent anywhere; the field is a scratch surface.
 */
public final class KeyboardTryoutActivity extends AppCompatActivity {

    /** The Apple screen's draft limit, kept so the two platforms bound the same way. */
    private static final int DRAFT_LIMIT = 2000;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final List<BackendAccount.ChatModel> models = new ArrayList<>();
    private final List<BackendAccount.ChatMessage> messages = new ArrayList<>();
    private Future<?> operation;
    private int generation;
    private boolean sending;

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        setContentView(R.layout.activity_keyboard_tryout);

        View root = findViewById(R.id.tryout_root);
        // The keyboard is the point of this screen, so its inset is applied rather than assumed:
        // the field has to stay above the input view as it is raised and dismissed.
        ViewCompat.setOnApplyWindowInsetsListener(root, (view, windowInsets) -> {
            Insets bars = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime());
            view.setPadding(bars.left, bars.top, bars.right, Math.max(bars.bottom, ime.bottom));
            return windowInsets;
        });

        MaterialToolbar bar = findViewById(R.id.tryout_bar);
        bar.setNavigationOnClickListener(ignored -> finish());

        TextInputEditText field = findViewById(R.id.tryout_field);
        MaterialButton dismiss = findViewById(R.id.tryout_dismiss);
        MaterialButton loadAi = findViewById(R.id.tryout_ai_load);
        MaterialButton sendAi = findViewById(R.id.tryout_ai_send);

        // The system picker belongs here rather than on the 键盘 tab: it is only useful once the
        // user is in front of an editor and finds another keyboard came up.
        MaterialButton switchIme = findViewById(R.id.tryout_switch);
        switchIme.setOnClickListener(ignored ->
            getSystemService(InputMethodManager.class).showInputMethodPicker());

        // 收起键盘 only means something while the keyboard is up, as on Apple.
        dismiss.setVisibility(View.GONE);
        dismiss.setOnClickListener(ignored -> {
            field.clearFocus();
            getSystemService(InputMethodManager.class).hideSoftInputFromWindow(field.getWindowToken(), 0);
        });
        field.setOnFocusChangeListener((view, focused) ->
            dismiss.setVisibility(focused ? View.VISIBLE : View.GONE));

        field.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence s, int start, int before, int count) { }
            @Override public void afterTextChanged(@NonNull Editable text) {
                if (text.length() > DRAFT_LIMIT) text.delete(DRAFT_LIMIT, text.length());
                sendAi.setEnabled(!sending && !models.isEmpty() && text.length() > 0);
            }
        });

        loadAi.setOnClickListener(ignored -> {
            if (models.isEmpty()) loadModels(loadAi, sendAi);
            else showModelMenu(loadAi, sendAi);
        });
        sendAi.setOnClickListener(ignored -> {
            if (sending) cancelChat(sendAi);
            else sendChat(field, sendAi);
        });

        // Opening the screen is the user asking for the keyboard, so it is raised without a tap.
        field.requestFocus();
        WindowCompat.getInsetsController(getWindow(), field).show(WindowInsetsCompat.Type.ime());
    }

    private void loadModels(MaterialButton load, MaterialButton send) {
        load.setEnabled(false);
        load.setText("加载中…");
        operation = worker.submit(() -> {
            try {
                List<BackendAccount.ChatModel> loaded = new BackendAccount(this).chatModels();
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    models.clear();
                    models.addAll(loaded);
                    load.setEnabled(true);
                    if (models.isEmpty()) {
                        load.setText("重新加载 AI");
                        send.setEnabled(false);
                    } else {
                        load.setText("模型 " + models.get(0).id());
                        // The draft may have been typed while the catalogue was loading. Refresh
                        // the action state here instead of waiting for another edit notification.
                        send.setEnabled(field.length() > 0);
                    }
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    load.setEnabled(true);
                    load.setText("重新加载 AI");
                    android.widget.Toast.makeText(this,
                        "请先登录账号，或稍后重试模型目录", android.widget.Toast.LENGTH_LONG).show();
                });
            }
        });
    }

    private void showModelMenu(MaterialButton load, MaterialButton send) {
        android.widget.PopupMenu menu = new android.widget.PopupMenu(this, load);
        for (BackendAccount.ChatModel model : models) {
            menu.getMenu().add(model.id()).setOnMenuItemClickListener(item -> {
                load.setText("模型 " + model.id());
                load.setTag(model.id());
                return true;
            });
        }
        menu.show();
    }

    private void sendChat(TextInputEditText field, MaterialButton send) {
        String text = field.getText() == null ? "" : field.getText().toString().trim();
        if (text.isEmpty() || models.isEmpty()) return;
        while (messages.size() >= 14) messages.remove(0);
        messages.add(new BackendAccount.ChatMessage("user", text));
        field.setText("");
        appendChat("你：" + text);
        sending = true;
        send.setEnabled(true);
        send.setText("停止");
        int token = ++generation;
        List<BackendAccount.ChatMessage> request = new ArrayList<>(messages);
        String selected = (String) findViewById(R.id.tryout_ai_load).getTag();
        if (selected == null || selected.isEmpty()) selected = models.get(0).id();
        final String selectedModel = selected;
        operation = worker.submit(() -> {
            try {
                String reply = new BackendAccount(this).chat(request, selectedModel);
                runOnUiThread(() -> {
                    if (token != generation) return;
                    messages.add(new BackendAccount.ChatMessage("assistant", reply));
                    appendChat("AI：" + reply);
                    finishChat(send);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (token != generation) return;
                    appendChat("AI：请求失败，请检查登录状态或稍后重试。");
                    finishChat(send);
                });
            }
        });
    }

    private void cancelChat(MaterialButton send) {
        generation++;
        if (operation != null) operation.cancel(true);
        finishChat(send);
    }

    private void finishChat(MaterialButton send) {
        sending = false;
        send.setText("发送");
        TextInputEditText field = findViewById(R.id.tryout_field);
        send.setEnabled(!models.isEmpty() && field.getText() != null && field.length() > 0);
    }

    private void appendChat(String line) {
        TextView transcript = findViewById(R.id.tryout_chat);
        transcript.setVisibility(View.VISIBLE);
        String prior = transcript.getText().toString();
        String next = prior.isEmpty() ? line : prior + "\n\n" + line;
        if (next.length() > 24_000) next = next.substring(next.length() - 24_000);
        transcript.setText(next);
    }

    @Override protected void onDestroy() {
        generation++;
        if (operation != null) operation.cancel(true);
        worker.shutdownNow();
        super.onDestroy();
    }
}
