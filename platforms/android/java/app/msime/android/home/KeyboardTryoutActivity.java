package app.msime.android.home;

import android.content.res.ColorStateList;
import android.os.Bundle;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.button.MaterialButton;
import app.msime.android.R;
import app.msime.android.BackendAccount;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

/**
 * 试用键盘：一个只为了把键盘调出来而存在的输入框，按设计做成聊天样式。
 *
 * <p>这里的键盘是绑定到这个输入框上的真正输入法，不是首页的静态预览。打的字不会被读取、保存或发送到任何地方；只有在用户登录、加载了 AI 模型并点发送之后，那一条消息才会作为 AI 对话发出去。没有加载模型时发送键保持禁用，对话区只有水杉的问候。
 */
public final class KeyboardTryoutActivity extends AppCompatActivity {

    /** The Apple screen's draft limit, kept so the two platforms bound the same way. */
    private static final int DRAFT_LIMIT = 2000;
    /** 对话区最多保留的气泡数，超过时从最早的开始丢。 */
    private static final int BUBBLE_LIMIT = 60;
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

        findViewById(R.id.tryout_back).setOnClickListener(ignored -> finish());

        EditText field = findViewById(R.id.tryout_field);
        MaterialButton dismiss = findViewById(R.id.tryout_dismiss);
        MaterialButton loadAi = findViewById(R.id.tryout_ai_load);
        MaterialButton sendAi = findViewById(R.id.tryout_ai_send);

        // 设计的输入栏：andCard 底、上面一条分隔线；输入框是页面底色的胶囊，描一圈 hair。
        View inputBar = findViewById(R.id.tryout_input_bar);
        inputBar.setBackgroundColor(Ui.card(this));
        // 固定 20 dp 圆角而不是全圆：单行 40 dp 高时看起来仍是胶囊，长到几行时是圆角矩形，不会撑成一个椭圆。
        android.graphics.drawable.GradientDrawable pill = Ui.rounded(Ui.page(this), Ui.dp(this, 20));
        pill.setStroke(Ui.dp(this, 1), Ui.hairline(this));
        field.setBackground(pill);
        // 聊天页的回车是发送：键盘回车显示「发送」，按下等同右边的发送键，不再插入换行把输入框越撑越高。长句仍会折行显示，最多 4 行。
        field.setHorizontallyScrolling(false);
        field.setMaxLines(4);
        field.setOnEditorActionListener((view, action, event) -> {
            if (action != android.view.inputmethod.EditorInfo.IME_ACTION_SEND) return false;
            if (sendAi.isEnabled()) sendAi.performClick();
            return true;
        });
        int accent = Ui.accent(this);
        sendAi.setBackgroundTintList(new ColorStateList(
            new int[][] {{-android.R.attr.state_enabled}, {}},
            new int[] {Ui.withAlpha(accent, 0.38f), accent}));

        // The system picker belongs here rather than on the home page: it is only useful once the
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
                // 请求进行中按钮是「停止」，继续编辑或清空草稿都不能禁用取消操作。
                sendAi.setEnabled(sending || (!models.isEmpty() && text.length() > 0));
            }
        });

        loadAi.setOnClickListener(ignored -> {
            if (models.isEmpty()) loadModels(field, loadAi, sendAi);
            else showModelMenu(loadAi, sendAi);
        });
        sendAi.setOnClickListener(ignored -> {
            if (sending) cancelChat(sendAi);
            else sendChat(field, sendAi);
        });
        findViewById(R.id.tryout_clear).setOnClickListener(ignored -> clear(field, sendAi));

        greet();

        // Opening the screen is the user asking for the keyboard, so it is raised without a tap.
        field.requestFocus();
        WindowCompat.getInsetsController(getWindow(), field).show(WindowInsetsCompat.Type.ime());
    }

    /** 第一条气泡：水杉的问候。 */
    private void greet() {
        appendBubble("你好，我是" + getString(R.string.app_name) + "。打几个字发给我试试，比如 shuishan。", false);
    }

    /** 清空：停掉进行中的请求，清掉草稿、对话和上下文，回到只有问候的样子。 */
    private void clear(EditText field, MaterialButton send) {
        if (sending) cancelChat(send);
        messages.clear();
        ((LinearLayout) findViewById(R.id.tryout_chat)).removeAllViews();
        field.setText("");
        greet();
    }

    private void loadModels(EditText field, MaterialButton load, MaterialButton send) {
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
                    MsToast.show(this, "请先登录账号，或稍后重试模型目录");
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

    private void sendChat(EditText field, MaterialButton send) {
        String text = field.getText() == null ? "" : field.getText().toString().trim();
        if (text.isEmpty() || models.isEmpty()) return;
        while (messages.size() >= 14) messages.remove(0);
        messages.add(new BackendAccount.ChatMessage("user", text));
        field.setText("");
        appendBubble(text, true);
        sending = true;
        send.setEnabled(true);
        send.setIconResource(R.drawable.ms_w2_home_stop);
        send.setContentDescription("停止");
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
                    appendBubble(reply, false);
                    finishChat(send);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (token != generation) return;
                    appendBubble("请求失败，请检查登录状态或稍后重试。", false);
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
        send.setIconResource(R.drawable.ms_w2_home_send);
        send.setContentDescription("发送");
        EditText field = findViewById(R.id.tryout_field);
        send.setEnabled(!models.isEmpty() && field.getText() != null && field.length() > 0);
    }

    /**
     * 加一个气泡：自己的消息靠右、accent 底 onAccent 字；水杉和 AI 的靠左、andCard 底。圆角 18，最宽到对话区的八成。
     */
    private void appendBubble(String text, boolean mine) {
        LinearLayout chat = findViewById(R.id.tryout_chat);
        while (chat.getChildCount() >= BUBBLE_LIMIT) chat.removeViewAt(0);
        TextView bubble = new TextView(this);
        bubble.setText(text.length() > 8_000 ? text.substring(0, 8_000) : text);
        bubble.setTextIsSelectable(true);
        Ui.style(bubble, 15, 400, mine ? Ui.onAccent(this) : Ui.text(this));
        bubble.setLineSpacing(Ui.dp(this, 3), 1f);
        bubble.setBackground(Ui.rounded(mine ? Ui.accent(this) : Ui.card(this), Ui.dp(this, 18)));
        bubble.setPadding(Ui.dp(this, 14), Ui.dp(this, 10), Ui.dp(this, 14), Ui.dp(this, 10));
        bubble.setMaxWidth(Math.round(getResources().getDisplayMetrics().widthPixels * 0.8f));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.gravity = mine ? Gravity.END : Gravity.START;
        if (chat.getChildCount() > 0) params.topMargin = Ui.dp(this, 10);
        chat.addView(bubble, params);
        ScrollView scroll = findViewById(R.id.tryout_scroll);
        // 只滚动不移焦点：fullScroll 会把焦点交给可选中的气泡，键盘就收起了。
        scroll.post(() -> scroll.smoothScrollTo(0, chat.getHeight()));
    }

    @Override protected void onDestroy() {
        generation++;
        if (operation != null) operation.cancel(true);
        worker.shutdownNow();
        super.onDestroy();
    }
}
