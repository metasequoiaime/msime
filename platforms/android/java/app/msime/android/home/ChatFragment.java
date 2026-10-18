package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.DimensionPolicy;

import app.msime.android.MainThreadPolicy;
import app.msime.android.ThreadPolicy;

import android.content.Context;
import android.os.Bundle;
import android.os.Handler;
import android.os.SystemClock;
import android.view.Gravity;
import android.view.LayoutInflater;
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
import app.msime.android.DrawablePolicy;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.WindowInsetsPolicy;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.button.MaterialButton;
import app.msime.android.R;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountStore;
import app.msime.android.RustAccount;
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
 * AI 对话，也是试用键盘：一个只为了把键盘调出来而存在的输入框，按设计做成聊天样式。
 *
 * <p>这里的键盘是绑定到这个输入框上的真正输入法，不是首页的静态预览。打的字不会被读取、保存或发送到任何地方；只有用户点了发送，那一条消息才会作为 AI 对话发给水杉的后端。对话经 Rust 的流式入口（`msime_client_account_chat_stream`，#8201 迁移计划 5c），它要求请求带 `model`：第一次发送前在工作线程上读一次 `chat_models` 的 `defaultModel`，用户在这里仍然选不了模型。没登录时发出去会得到「请先登录」的提示，对话区开头只有水杉的问候。
 *
 * <p>页面绑定打开时登录的账号编号：读登录状态要等 Rust 的会话锁，所以绑定和核对都在工作线程上做，界面线程不调 Rust。Rust 只在回答完整以后核对账号，生成中途退出登录时由 {@link SignIn#signOut} 取消这次对话（{@link RustAccount#cancelForSignOut}），回来的 account_cancelled 让页面回到刚打开的样子。
 *
 * <p>宿主通过设置导航栈打开本二级页，保留返回按钮；「试用键盘」还会请求输入框焦点。独立的 {@link KeyboardTryoutActivity} 继续承载共享设置发起的试用。
 */
public final class ChatFragment extends HomeTabFragment {

    /** boolean：由 {@link KeyboardTryoutActivity} 单独承载，而不是作为底部 tab。 */
    static final String ARG_STANDALONE = "standalone";
    /** 和 Apple 试用页同一个草稿上限，两个平台按同样的边界截断。 */
    private static final int DRAFT_LIMIT = 2000;
    /** 对话区最多保留的气泡数，超过时从最早的开始丢。 */
    private static final int BUBBLE_LIMIT = 60;
    /** 流式回复两次重画之间的最短间隔：最多每秒 20 次。 */
    private static final long STREAM_FRAME_MS = 50;
    /** 一条回复的 UTF-8 字节上限，与 Rust 的流式对话相同。 */
    private static final int MAX_REPLY_BYTES = 16 * 1024;
    /** 每次请求开头的系统提示词，与共享设置页的 packages/ui/src/chat/chat-prompt.ts 和 iOS 的 KeyboardChatHistoryPolicy 逐字相同，由 scripts/test-chat-system-prompt-parity.py 检查。 */
    private static final String SYSTEM_PROMPT =
        "你是水杉输入法（MSIME，Metasequoia IME）里的 AI 助手。用户在水杉输入法的应用里和你对话，多半是一边聊一边试键盘：看候选、皮肤和手感。\n\n介绍自己时说你是水杉输入法的 AI 助手，不要主动报出背后的模型名称或开发公司；用户直接问用的是什么模型时，如实回答。\n\n用户发来的常常是试打的内容：一个词、半句话、没转成汉字的拼音（比如 shuishan）、错字或随手敲的字母。这时简短自然地接话，看得出是拼音就顺带说出可能想打的字，不要追问用意，也不要长篇解释。\n\n关于水杉输入法：它是开源的中文输入法，支持 Android、iOS、macOS、Linux、Windows 和 HarmonyOS，官网 msime.app。你没有它完整的功能和设置资料，被问到具体功能、设置项或操作步骤时只说你确定的部分，其余建议用户在设置里查看或到官网了解，不要编造功能、菜单名称或步骤。你看不到用户的屏幕，也不能更改任何设置。\n\n隐私：这段对话会发给 AI 服务来生成回复，不计入输入统计；用户平时用输入法打的字不会发给你。\n\n回答：默认用简体中文；用户整句用其他语言时，用那种语言回答。回答简洁，先说结论，适合在小屏幕上读，少用标题和表格。";
    private final Handler mainHandler = MainThreadPolicy.mainHandler();
    private final ExecutorService worker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedFactory("msime-keyboard-tryout"));
    /** 对话里的一条：role 是 system、user 或 assistant。 */
    private record Turn(String role, String content) {}

    private final List<Turn> messages = new ArrayList<>(13);
    /** chat_models 给的默认模型；第一次发送前在工作线程上读，读到之前为 null。 */
    private volatile String model;
    private Future<?> operation;
    private int generation;
    private boolean sending;
    /** 页面绑定的账号编号；空值表示绑定时没有登录。在工作线程上读出，再在界面线程上写。 */
    private String pageSessionId = "";
    /** 工作线程上最近一次绑定读到的账号编号。打开页面后立刻发送时界面线程上的 {@link #pageSessionId} 可能还没写上，发送和绑定排在同一条工作线程上，发送时读它就一定是绑定之后的值。 */
    private volatile String boundUser = "";
    /** 正在流式到达的那条回复；没有请求在进行时为 null。只在界面线程上读写。 */
    private StreamingReply streaming;
    private boolean standalone;
    @Nullable private EditText field;
    @Nullable private MaterialButton sendAi;
    @Nullable private LinearLayout chat;
    @Nullable private ScrollView scroll;

    /** 给共享设置的独立试用 Activity 使用。 */
    static ChatFragment standalone() {
        ChatFragment fragment = new ChatFragment();
        Bundle arguments = new Bundle();
        arguments.putBoolean(ARG_STANDALONE, true);
        fragment.setArguments(arguments);
        return fragment;
    }

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_chat, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        Context context = requireContext();
        standalone = getArguments() != null && getArguments().getBoolean(ARG_STANDALONE);
        bindSession();
        field = view.findViewById(R.id.tryout_field);
        sendAi = view.findViewById(R.id.tryout_ai_send);
        chat = view.findViewById(R.id.tryout_chat);
        scroll = view.findViewById(R.id.tryout_scroll);
        EditText field = this.field;
        MaterialButton sendAi = this.sendAi;
        MaterialButton dismiss = view.findViewById(R.id.tryout_dismiss);

        // 键盘是这一页的重点，所以按实际 inset 让位而不是假设一个值：键盘升起和收起时输入框都要留在它上面。单独承载时自己让开系统栏；作为二级页时 HomeActivity 已经给内容区垫了顶部和左右，并在这一页收起底部导航（HomeActivity.updateTabsVisibility），底部只让开系统导航栏和输入法。
        ViewCompat.setOnApplyWindowInsetsListener(view, (target, windowInsets) -> {
            Insets bars = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime());
            int bottom = WindowInsetsPolicy.bottomContentInset(bars.bottom, 0, ime.bottom, 0);
            if (standalone) ViewPolicy.setPadding(target, bars.left, bars.top, bars.right, bottom);
            else ViewPolicy.setPadding(target, 0, 0, 0, bottom);
            return windowInsets;
        });
        ViewCompat.requestApplyInsets(view);

        TextView back = view.findViewById(R.id.tryout_back);
        TextView title = view.findViewById(R.id.tryout_title);
        if (standalone) {
            ViewPolicy.bindClick(back, () -> requireActivity().finish());
        } else {
            ViewPolicy.bindClick(back, () -> requireActivity().getOnBackPressedDispatcher().onBackPressed());
            title.setText("AI 对话");
        }

        // 设计的输入栏：andCard 底、上面一条分隔线；输入框是页面底色的胶囊，描一圈 hair。
        View inputBar = view.findViewById(R.id.tryout_input_bar);
        ViewPolicy.setBackgroundColor(inputBar, ThemeColorPolicy.card(context));
        // 固定 20 dp 圆角而不是全圆：单行 40 dp 高时看起来仍是胶囊，长到几行时是圆角矩形，不会撑成一个椭圆。
        android.graphics.drawable.GradientDrawable pill = DrawablePolicy.outlined(ThemeColorPolicy.page(context),
            DimensionPolicy.pixels(context, 20), DimensionPolicy.pixels(context, 1), ThemeColorPolicy.hairline(context));
        ViewPolicy.setBackground(field, pill);
        // 聊天页的回车是发送：键盘回车显示「发送」，按下等同右边的发送键，不再插入换行把输入框越撑越高。长句仍会折行显示，最多 4 行。
        field.setHorizontallyScrolling(false);
        ViewPolicy.setMaxLines(field, 4);
        field.setOnEditorActionListener((editor, action, event) -> {
            if (action != android.view.inputmethod.EditorInfo.IME_ACTION_SEND) return false;
            // 请求进行中按钮是「停止」：回车不去点它，只有点按钮才停。
            if (!sending && sendAi.isEnabled()) sendAi.performClick();
            return true;
        });
        int accent = ThemeColorPolicy.accent(context);
        sendAi.setBackgroundTintList(ColorPolicy.stateList(
            new int[][] {{-android.R.attr.state_enabled}, {}},
            new int[] {ColorPolicy.withAlpha(accent, 0.38f), accent}));

        // 系统的输入法选择器放在这里而不是首页：只有用户已经站在一个输入框前、发现弹出来的是别的键盘时，它才有用。
        MaterialButton switchIme = view.findViewById(R.id.tryout_switch);
        ViewPolicy.bindClick(switchIme, () ->
            requireContext().getSystemService(InputMethodManager.class).showInputMethodPicker());

        // 「收起键盘」只在键盘升起时有意义，和 Apple 一样。
        ViewPolicy.hide(dismiss);
        ViewPolicy.bindClick(dismiss, this::hideKeyboard);
        field.setOnFocusChangeListener((editor, focused) -> {
            if (focused) ViewPolicy.show(dismiss);
            else ViewPolicy.hide(dismiss);
        });

        ViewPolicy.afterTextChanged(field, text -> {
            if (text.length() > DRAFT_LIMIT) text.delete(DRAFT_LIMIT, text.length());
            // 请求进行中按钮是「停止」，继续编辑或清空草稿都不能禁用取消操作。默认就能和 AI 对话：有字就能发。
            ViewPolicy.setEnabled(sendAi, sending || text.length() > 0);
        });

        ViewPolicy.bindClick(sendAi, () -> {
            if (sending) cancelChat(sendAi);
            else sendChat(field, sendAi);
        });
        ViewPolicy.bindClick(view.findViewById(R.id.tryout_clear), () -> clear(field, sendAi));

        greet();

        // 单独打开这一页就是用户在要键盘，所以不等点按就把它弹出来；tab 由 HomeActivity 决定什么时候弹（见 focusInput）。
        if (standalone) focusInput();
    }

    /** 把焦点交给输入框并弹出键盘：单独承载时一打开就调用，tab 上由设置首页的「试用键盘」经 {@link HomeActivity#openChat} 调用。 */
    void focusInput() {
        EditText field = this.field;
        if (field == null) return;
        field.requestFocus();
        WindowCompat.getInsetsController(requireActivity().getWindow(), field).show(WindowInsetsCompat.Type.ime());
    }

    private void hideKeyboard() {
        EditText field = this.field;
        if (field == null) return;
        field.clearFocus();
        requireContext().getSystemService(InputMethodManager.class).hideSoftInputFromWindow(field.getWindowToken(), 0);
    }

    /** 每次回到眼前都核对一次账号：在「我的」里登录、退出或换了账号之后，这一页不能还拿着上一个账号的会话。核对在工作线程上，变了才回到界面线程重置。 */
    @Override protected void onBecameVisible() {
        if (field == null) return;
        String bound = pageSessionId;
        int token = generation;
        Context app = requireContext().getApplicationContext();
        worker.submit(() -> {
            if (bound.equals(currentUser(app))) return;
            mainHandler.post(() -> {
                if (token == generation && alive() && bound.equals(pageSessionId)) abortForSessionChange();
            });
        });
    }

    /** 在工作线程上读出现在登录的账号，回到界面线程记成页面绑定的账号。 */
    private void bindSession() {
        Context app = requireContext().getApplicationContext();
        int token = generation;
        worker.submit(() -> {
            String user = currentUser(app);
            boundUser = user;
            mainHandler.post(() -> {
                if (token == generation && alive()) pageSessionId = user;
            });
        });
    }

    /** 现在登录的账号编号；没有登录或读不了时为空串。阻塞，只在工作线程上调用。 */
    private static String currentUser(Context app) {
        try {
            return RustAccount.userId(app);
        } catch (AccountStore.Failure unavailable) {
            return "";
        }
    }

    @Override public void onHiddenChanged(boolean hidden) {
        super.onHiddenChanged(hidden);
        // 切到别的 tab 时把键盘收起来，不然它会盖在另一个 tab 上。
        if (hidden) {
            hideKeyboard();
        }
    }

    /** 第一条气泡：水杉的问候。 */
    private void greet() {
        appendBubble("你好，我是" + getString(R.string.app_name) + "。打几个字发给我试试，比如 shuishan。", false);
    }

    /** 清空：停掉进行中的请求，清掉草稿、对话和上下文，回到只有问候的样子。 */
    private void clear(EditText field, MaterialButton send) {
        if (sending) cancelChat(send);
        messages.clear();
        if (chat != null) chat.removeAllViews();
        field.setText("");
        greet();
    }

    private void sendChat(EditText field, MaterialButton send) {
        String text = TextPolicy.trimmed(field.getText() == null ? null : field.getText().toString());
        if (text.isEmpty()) return;
        field.setText("");
        appendBubble(text, true);
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
        messages.add(new Turn("user", text));
        showStop(send);
        int token = ++generation;
        List<Turn> request = new ArrayList<>(messages.size() + 1);
        request.add(new Turn("system", SYSTEM_PROMPT));
        request.addAll(messages);
        final String session = pageSessionId;
        Context app = requireContext().getApplicationContext();
        StreamingReply reply = new StreamingReply(token);
        streaming = reply;
        operation = worker.submit(() -> {
            try {
                String full = stream(app, session, request, reply);
                mainHandler.post(() -> {
                    if (token != generation || !alive()) return;
                    streaming = null;
                    reply.show(full);
                    messages.add(new Turn("assistant", full));
                    finishChat(send);
                });
            } catch (AccountStore.Failure failure) {
                mainHandler.post(() -> {
                    if (token != generation || !alive()) return;
                    // 期间退出或换了账号（Rust 回答完整后核对，或退出时被取消）：已经显示的那段也丢掉，页面改绑现在的账号。
                    if ("account_cancelled".equals(failure.code)) {
                        abortForSessionChange();
                        return;
                    }
                    streaming = null;
                    // 已经到了一部分就留着它；一个字都没到才说失败。
                    if (!keepPartial(reply))
                        appendBubble(AccountFailureText.chat(failure.code, failure.retryAfterSeconds), false);
                    finishChat(send);
                });
            }
        });
    }

    /**
     * 工作线程：核对页面绑定的账号仍在、取默认模型，再发流式对话。没登录时不发请求，以 account_unauthorized 失败；账号变了是 account_cancelled。
     */
    private String stream(Context app, String session, List<Turn> request, StreamingReply reply)
            throws AccountStore.Failure {
        String bound = session.isEmpty() ? boundUser : session;
        if (bound.isEmpty()) throw new AccountStore.Failure("account_unauthorized", 0);
        RustAccount.requireUser(app, bound);
        String chosen = model;
        if (chosen == null) {
            chosen = defaultModel(app);
            model = chosen;
        }
        org.json.JSONArray body = new org.json.JSONArray();
        try {
            for (Turn turn : request) {
                body.put(new org.json.JSONObject().put("role", turn.role()).put("content", turn.content()));
            }
        } catch (org.json.JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        return RustAccount.chatStream(app, body, chosen, reply.call, reply::append);
    }

    /** chat_models 的 defaultModel：Rust 的流式对话要求请求带模型，用户在这里选不了，就用服务端说的默认那一个。 */
    private static String defaultModel(Context app) throws AccountStore.Failure {
        org.json.JSONObject catalog = RustAccount.accountObject(app, chatModels());
        String chosen = catalog.optString("defaultModel", "");
        if (chosen.isEmpty()) throw new AccountStore.Failure("account_unavailable", 0);
        return chosen;
    }

    private static org.json.JSONObject chatModels() throws AccountStore.Failure {
        try {
            return new org.json.JSONObject().put("operation", "chat_models");
        } catch (org.json.JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    private void cancelChat(MaterialButton send) {
        StreamingReply reply = streaming;
        streaming = null;
        generation++;
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
        messages.add(new Turn("assistant", partial));
        return true;
    }

    /**
     * 正在流式到达的一条回复。worker 线程把增量追加进 {@code received}，界面线程最多每 {@link #STREAM_FRAME_MS} 毫秒重画一次气泡；第一段增量到达时才创建气泡。
     */
    private final class StreamingReply {
        final int token;
        final RustAccount.Call call = new RustAccount.Call();
        private final StringBuilder received = new StringBuilder(MAX_REPLY_BYTES);
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
            if (token != generation || !alive()) return;
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
        EditText field = this.field;
        ViewPolicy.setEnabled(send, field != null && field.getText() != null && field.length() > 0);
    }

    /**
     * 页面所属账号已经改变：丢弃草稿和聊天流。单独承载时关闭页面；tab 关不掉，就回到刚打开的样子，改绑现在的账号。
     */
    private void abortForSessionChange() {
        generation++;
        if (streaming != null) streaming.call.cancel();
        streaming = null;
        if (operation != null) operation.cancel(true);
        if (standalone) {
            requireActivity().finish();
            return;
        }
        EditText field = this.field;
        MaterialButton send = sendAi;
        if (field == null || send == null) return;
        pageSessionId = "";
        bindSession();
        messages.clear();
        if (chat != null) chat.removeAllViews();
        field.setText("");
        finishChat(send);
        greet();
    }

    /** 回调到达时这一页还在：已经加在界面上、视图也还在。 */
    private boolean alive() {
        return isAdded() && field != null;
    }

    /**
     * 加一个气泡：自己的消息靠右、accent 底 onAccent 字；水杉和 AI 的靠左、andCard 底。圆角 18，最宽到对话区的八成。
     */
    private TextView appendBubble(String text, boolean mine) {
        Context context = requireContext();
        LinearLayout chat = this.chat;
        while (chat.getChildCount() >= BUBBLE_LIMIT) chat.removeViewAt(0);
        TextView bubble = ViewPolicy.styledLabel(context, text, 15, 400,
            mine ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.text(context));
        // 先设可选再放文字：setTextIsSelectable 会换成 ArrowKeyMovementMethod，放在后面就把 Markwon 装好的 LinkMovementMethod 冲掉，回复里的链接点不动。AI 的气泡再显式装上链接的点按处理，之后流式更新的 setMarkdown 会沿用它。
        bubble.setTextIsSelectable(true);
        setBubbleText(bubble, text, !mine);
        if (!mine) bubble.setMovementMethod(android.text.method.LinkMovementMethod.getInstance());
        ViewPolicy.setLineSpacing(bubble, DimensionPolicy.pixels(context, 3), 1f);
        ViewPolicy.setBackground(bubble, DrawablePolicy.rounded(mine ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.card(context), DimensionPolicy.pixels(context, 18)));
        ViewPolicy.setSymmetricPaddingDp(context, bubble, 14, 10);
        bubble.setMaxWidth(Math.round(DimensionPolicy.screenWidthPixels(context) * 0.8f));
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.gravity = mine ? Gravity.END : Gravity.START;
        if (chat.getChildCount() > 0) params.topMargin = DimensionPolicy.pixels(context, 10);
        chat.addView(bubble, params);
        scrollToEnd();
        return bubble;
    }

    /** AI 与水杉的气泡按 Markdown 渲染（加粗、列表、标题、引用、代码、链接）；自己发的那句原样显示。 */
    private void setBubbleText(TextView bubble, String text, boolean markdown) {
        String shown = TextPolicy.clipSurrogateSafe(text, 8_000);
        if (markdown) markwon().setMarkdown(bubble, shown);
        else bubble.setText(shown);
    }

    private Markwon markwon;

    /** 只用 Markwon 的核心：不解释 HTML、不加载图片；链接与公告页一样只打开 http、https 和 mailto。 */
    private Markwon markwon() {
        if (markwon == null) {
            markwon = Markwon.builder(requireContext())
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
        LinkActions.openMarkdown(requireContext(), link);
    }

    private void scrollToEnd() {
        LinearLayout chat = this.chat;
        ScrollView scroll = this.scroll;
        if (chat == null || scroll == null) return;
        // 只滚动不移焦点：fullScroll 会把焦点交给可选中的气泡，键盘就收起了。
        scroll.post(() -> scroll.smoothScrollTo(0, chat.getHeight()));
    }

    @Override public void onDestroyView() {
        generation++;
        sending = false;
        if (streaming != null) streaming.call.cancel();
        streaming = null;
        mainHandler.removeCallbacksAndMessages(null);
        if (operation != null) operation.cancel(true);
        field = null;
        sendAi = null;
        chat = null;
        scroll = null;
        markwon = null;
        super.onDestroyView();
    }

    @Override public void onDestroy() {
        worker.shutdownNow();
        super.onDestroy();
    }
}
