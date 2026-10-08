package app.msime.android.home;

import app.msime.android.TextPolicy;

import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
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
import app.msime.android.ColorPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.button.MaterialButton;
import app.msime.android.R;
import app.msime.android.BackendAccount;
import io.noties.markwon.AbstractMarkwonPlugin;
import io.noties.markwon.Markwon;
import io.noties.markwon.MarkwonConfiguration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicBoolean;

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
    /** 流式回复两次重画之间的最短间隔：最多每秒 20 次。 */
    private static final long STREAM_FRAME_MS = 50;
    private static final String FAILURE = "请求失败，请检查登录状态或稍后重试。";
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final ArrayList<BackendAccount.ChatModel> models = new ArrayList<>(BackendAccount.MAX_CHAT_MODELS);
    private final List<BackendAccount.ChatMessage> messages = new ArrayList<>(13);
    private Future<?> operation;
    private int generation;
    private boolean sending;
    /** 模型目录正在加载。 */
    private boolean loadingModels;
    /** 目录还没到时就发出的一句：已经画成气泡，目录到了再真正发给模型。 */
    private String pendingSend;
    /** 正在流式到达的那条回复；没有请求在进行时为 null。只在界面线程上读写。 */
    private StreamingReply streaming;

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
            ViewPolicy.setPadding(view, bars.left, bars.top, bars.right,
                Ui.bottomContentInset(bars.bottom, 0, ime.bottom, 0));
            return windowInsets;
        });

        ViewPolicy.bindClick(findViewById(R.id.tryout_back), this::finish);

        EditText field = findViewById(R.id.tryout_field);
        MaterialButton dismiss = findViewById(R.id.tryout_dismiss);
        MaterialButton sendAi = findViewById(R.id.tryout_ai_send);

        // 设计的输入栏：andCard 底、上面一条分隔线；输入框是页面底色的胶囊，描一圈 hair。
        View inputBar = findViewById(R.id.tryout_input_bar);
        ViewPolicy.setBackgroundColor(inputBar, Ui.card(this));
        // 固定 20 dp 圆角而不是全圆：单行 40 dp 高时看起来仍是胶囊，长到几行时是圆角矩形，不会撑成一个椭圆。
        android.graphics.drawable.GradientDrawable pill = Ui.outlined(Ui.page(this),
            Ui.dp(this, 20), Ui.dp(this, 1), Ui.hairline(this));
        ViewPolicy.setBackground(field, pill);
        // 聊天页的回车是发送：键盘回车显示「发送」，按下等同右边的发送键，不再插入换行把输入框越撑越高。长句仍会折行显示，最多 4 行。
        field.setHorizontallyScrolling(false);
        ViewPolicy.setMaxLines(field, 4);
        field.setOnEditorActionListener((view, action, event) -> {
            if (action != android.view.inputmethod.EditorInfo.IME_ACTION_SEND) return false;
            // 请求进行中按钮是「停止」：回车不去点它，只有点按钮才停。
            if (!sending && sendAi.isEnabled()) sendAi.performClick();
            return true;
        });
        int accent = Ui.accent(this);
        sendAi.setBackgroundTintList(ColorPolicy.stateList(
            new int[][] {{-android.R.attr.state_enabled}, {}},
            new int[] {Ui.withAlpha(accent, 0.38f), accent}));

        // The system picker belongs here rather than on the home page: it is only useful once the
        // user is in front of an editor and finds another keyboard came up.
        MaterialButton switchIme = findViewById(R.id.tryout_switch);
        ViewPolicy.bindClick(switchIme, () ->
            getSystemService(InputMethodManager.class).showInputMethodPicker());

        // 收起键盘 only means something while the keyboard is up, as on Apple.
        ViewPolicy.hide(dismiss);
        ViewPolicy.bindClick(dismiss, () -> {
            field.clearFocus();
            getSystemService(InputMethodManager.class).hideSoftInputFromWindow(field.getWindowToken(), 0);
        });
        field.setOnFocusChangeListener((view, focused) -> {
            if (focused) ViewPolicy.show(dismiss);
            else ViewPolicy.hide(dismiss);
        });

        field.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence s, int start, int before, int count) { }
            @Override public void afterTextChanged(@NonNull Editable text) {
                if (text.length() > DRAFT_LIMIT) text.delete(DRAFT_LIMIT, text.length());
                // 请求进行中按钮是「停止」，继续编辑或清空草稿都不能禁用取消操作。默认就能和 AI 对话：有字就能发，目录还没加载完时发出的那句等目录到了再发。
                ViewPolicy.setEnabled(sendAi, sending || text.length() > 0);
            }
        });

        ViewPolicy.bindClick(sendAi, () -> {
            if (sending) cancelChat(sendAi);
            else sendChat(field, sendAi);
        });
        ViewPolicy.bindClick(findViewById(R.id.tryout_clear), () -> clear(field, sendAi));

        greet();
        // 进页面就在后台加载模型目录，不再要用户先点「加载 AI」。
        loadModels(field, sendAi);

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
        pendingSend = null;
        messages.clear();
        ((LinearLayout) findViewById(R.id.tryout_chat)).removeAllViews();
        field.setText("");
        greet();
    }

    /** 后台加载模型目录；成功后刷新发送键，并把目录到之前发出的那句真正发出去。 */
    private void loadModels(EditText field, MaterialButton send) {
        if (loadingModels) return;
        loadingModels = true;
        operation = worker.submit(() -> {
            try {
                List<BackendAccount.ChatModel> loaded = new BackendAccount(this).chatModels();
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    models.clear();
                    models.ensureCapacity(loaded.size());
                    models.addAll(loaded);
                    loadingModels = false;
                    // The draft may have been typed while the catalogue was loading. Refresh
                    // the action state here instead of waiting for another edit notification.
                    ViewPolicy.setEnabled(send, sending || field.length() > 0);
                    flushPendingSend(send);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    loadingModels = false;
                    flushPendingSend(send);
                });
            }
        });
    }

    /** 目录到了（或没拿到）时处理等着的那句：有模型就发，没有就在对话里说明。 */
    private void flushPendingSend(MaterialButton send) {
        String text = pendingSend;
        pendingSend = null;
        if (text == null) return;
        if (models.isEmpty()) {
            appendBubble(FAILURE, false);
            finishChat(send);
            return;
        }
        dispatchChat(text, send);
    }

    private void sendChat(EditText field, MaterialButton send) {
        String text = TextPolicy.trimmed(field.getText() == null ? null : field.getText().toString());
        if (text.isEmpty()) return;
        field.setText("");
        appendBubble(text, true);
        if (models.isEmpty()) {
            // 目录还没到：这句先画出来，按钮变成「停止」，目录到了再发。
            pendingSend = text;
            showStop(send);
            loadModels(field, send);
            return;
        }
        dispatchChat(text, send);
    }

    private void showStop(MaterialButton send) {
        sending = true;
        ViewPolicy.setEnabled(send, true);
        send.setIconResource(R.drawable.ms_w2_home_stop);
        send.setContentDescription("停止");
    }

    private void dispatchChat(String text, MaterialButton send) {
        // 请求最多 14 条：开头一条系统约定，加最近 13 条对话。
        while (messages.size() >= 13) messages.remove(0);
        messages.add(new BackendAccount.ChatMessage("user", text));
        showStop(send);
        int token = ++generation;
        List<BackendAccount.ChatMessage> request = new ArrayList<>(messages.size() + 1);
        request.add(new BackendAccount.ChatMessage("system", SYSTEM_PROMPT));
        request.addAll(messages);
        final String selectedModel = models.get(0).id();
        StreamingReply reply = new StreamingReply(token);
        streaming = reply;
        operation = worker.submit(() -> {
            try {
                String full = new BackendAccount(this).chatStream(request, selectedModel, reply.call, reply::append);
                runOnUiThread(() -> {
                    if (token != generation) return;
                    streaming = null;
                    reply.show(full);
                    messages.add(new BackendAccount.ChatMessage("assistant", full));
                    finishChat(send);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (token != generation) return;
                    streaming = null;
                    // 已经到了一部分就留着它；一个字都没到才说失败。
                    if (!keepPartial(reply)) appendBubble(FAILURE, false);
                    finishChat(send);
                });
            }
        });
    }

    private void cancelChat(MaterialButton send) {
        StreamingReply reply = streaming;
        streaming = null;
        generation++;
        pendingSend = null;
        if (reply != null) {
            reply.call.cancel();
            // 停止时保留已经到达的部分，并让它进入上下文，和屏幕上看到的一致。
            keepPartial(reply);
        }
        if (operation != null) operation.cancel(true);
        finishChat(send);
    }

    /** 把一条没收完的回复里已经到达的文字画出来并记进上下文；一个字都没到时返回 false。 */
    private boolean keepPartial(StreamingReply reply) {
        String partial = reply.text();
        if (partial.isEmpty()) return false;
        reply.show(partial);
        messages.add(new BackendAccount.ChatMessage("assistant", partial));
        return true;
    }

    /**
     * 正在流式到达的一条回复。worker 线程把增量追加进 {@code received}，界面线程最多每 {@link #STREAM_FRAME_MS} 毫秒重画一次气泡；第一段增量到达时才创建气泡。
     */
    private final class StreamingReply {
        final int token;
        final BackendAccount.ChatCall call = new BackendAccount.ChatCall();
        private final StringBuilder received = new StringBuilder(BackendAccount.MAX_CHAT_REPLY_BYTES);
        private final AtomicBoolean scheduled = new AtomicBoolean();
        /** 上一次重画的时刻；worker 线程读它算延迟，界面线程写。 */
        private volatile long shownAt;
        /** 只在界面线程上读写。 */
        private TextView bubble;

        StreamingReply(int token) {
            this.token = token;
        }

        /** worker 线程：追加一段增量，没有排着的重画就排一次。 */
        void append(String delta) {
            synchronized (received) {
                received.append(delta);
            }
            if (scheduled.compareAndSet(false, true)) {
                long wait = BoundsPolicy.nonNegative(
                    shownAt + STREAM_FRAME_MS - SystemClock.uptimeMillis());
                mainHandler.postDelayed(this::render, wait);
            }
        }

        String text() {
            synchronized (received) {
                return received.toString();
            }
        }

        /** 界面线程：先清掉排队标记再取文字，这之后到的增量会再排一次重画，不会丢。 */
        private void render() {
            scheduled.set(false);
            if (token != generation || isFinishing() || isDestroyed()) return;
            show(text());
        }

        /** 界面线程：把气泡更新成 {@code text}，第一次调用时创建气泡。 */
        void show(String text) {
            if (text.isEmpty()) return;
            shownAt = SystemClock.uptimeMillis();
            if (bubble == null) {
                bubble = appendBubble(text, false);
            } else {
                setBubbleText(bubble, text, true);
                scrollToEnd();
            }
        }
    }

    private void finishChat(MaterialButton send) {
        sending = false;
        send.setIconResource(R.drawable.ms_w2_home_send);
        send.setContentDescription("发送");
        EditText field = findViewById(R.id.tryout_field);
        ViewPolicy.setEnabled(send, field.getText() != null && field.length() > 0);
    }

    /**
     * 加一个气泡：自己的消息靠右、accent 底 onAccent 字；水杉和 AI 的靠左、andCard 底。圆角 18，最宽到对话区的八成。
     */
    /** 每次请求开头的系统约定：默认用简体中文回答。 */
    private static final String SYSTEM_PROMPT =
        "你是水杉输入法里的 AI 助手。除非用户明确要求使用其他语言，一律用简体中文回答，回答简洁。";

    private TextView appendBubble(String text, boolean mine) {
        LinearLayout chat = findViewById(R.id.tryout_chat);
        while (chat.getChildCount() >= BUBBLE_LIMIT) chat.removeViewAt(0);
        TextView bubble = Ui.styledLabel(this, text, 15, 400,
            mine ? Ui.onAccent(this) : Ui.text(this));
        // 先设可选再放文字：setTextIsSelectable 会换成 ArrowKeyMovementMethod，放在后面就把 Markwon 装好的 LinkMovementMethod 冲掉，回复里的链接点不动。AI 的气泡再显式装上链接的点按处理，之后流式更新的 setMarkdown 会沿用它。
        bubble.setTextIsSelectable(true);
        setBubbleText(bubble, text, !mine);
        if (!mine) bubble.setMovementMethod(android.text.method.LinkMovementMethod.getInstance());
        ViewPolicy.setLineSpacing(bubble, Ui.dp(this, 3), 1f);
        ViewPolicy.setBackground(bubble, Ui.rounded(mine ? Ui.accent(this) : Ui.card(this), Ui.dp(this, 18)));
        Ui.setSymmetricPaddingDp(bubble, this, 14, 10);
        bubble.setMaxWidth(Math.round(Ui.screenWidthPixels(this) * 0.8f));
        LinearLayout.LayoutParams params = Ui.wrap();
        params.gravity = mine ? Gravity.END : Gravity.START;
        if (chat.getChildCount() > 0) params.topMargin = Ui.dp(this, 10);
        chat.addView(bubble, params);
        scrollToEnd();
        return bubble;
    }

    /** AI 与水杉的气泡按 Markdown 渲染（加粗、列表、标题、引用、代码、链接）；自己发的那句原样显示。 */
    private void setBubbleText(TextView bubble, String text, boolean markdown) {
        String shown = TextPolicy.clip(text, 8_000);
        if (markdown) markwon().setMarkdown(bubble, shown);
        else bubble.setText(shown);
    }

    private Markwon markwon;

    /** 只用 Markwon 的核心：不解释 HTML、不加载图片；链接与公告页一样只打开 http、https 和 mailto。 */
    private Markwon markwon() {
        if (markwon == null) {
            markwon = Markwon.builder(this)
                .usePlugin(new AbstractMarkwonPlugin() {
                    @Override public void configureConfiguration(@NonNull MarkwonConfiguration.Builder builder) {
                        builder.linkResolver((view, link) -> openLink(link));
                    }
                })
                .build();
        }
        return markwon;
    }

    private void openLink(String link) {
        android.net.Uri uri = android.net.Uri.parse(link);
        String scheme = uri.getScheme() == null ? "" : uri.getScheme().toLowerCase(java.util.Locale.ROOT);
        if (!scheme.equals("https") && !scheme.equals("http") && !scheme.equals("mailto")) return;
        try {
            startActivity(new android.content.Intent(android.content.Intent.ACTION_VIEW, uri));
        } catch (RuntimeException error) {
            // 没有浏览器时链接只是文字。
        }
    }

    private void scrollToEnd() {
        LinearLayout chat = findViewById(R.id.tryout_chat);
        ScrollView scroll = findViewById(R.id.tryout_scroll);
        // 只滚动不移焦点：fullScroll 会把焦点交给可选中的气泡，键盘就收起了。
        scroll.post(() -> scroll.smoothScrollTo(0, chat.getHeight()));
    }

    @Override protected void onDestroy() {
        generation++;
        if (streaming != null) streaming.call.cancel();
        streaming = null;
        mainHandler.removeCallbacksAndMessages(null);
        if (operation != null) operation.cancel(true);
        worker.shutdownNow();
        super.onDestroy();
    }
}
