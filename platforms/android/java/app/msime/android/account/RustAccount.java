package app.msime.android;

import android.content.Context;
import java.nio.charset.StandardCharsets;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 设置页面经 Rust 的账号与云端入口（#8201 迁移计划 5c）：账号页、我的设备、云端数据、反馈、诊断上传、下载链接、社区、对话、AI 皮肤和云剪贴板里不涉及内容的几个操作。请求一律带 {@link AccountStore} 注册的 `session_store`，令牌、刷新和 401 后的重试都在 Rust 里，Java 只拼请求、读回答。
 *
 * <p>页面按打开时的账号编号（user.id）绑定：C 接口拿不到会话代次，所以请求前后各用 {@link #requireUser} 读一次 Rust 的 status，退出或换了号就以 account_cancelled 丢掉结果。
 *
 * <p>只在主进程里用（`:ime` 不注册存储，{@link AccountStore#id} 会抛出）。除 {@link #userId}、{@link #chatCancel} 和 {@link #aiSkinCancel} 外都阻塞网络，不要在主线程上调用；{@link #userId} 不联网，但要等 Rust 的会话锁，同样放在工作线程上。
 */
public final class RustAccount {
    /** 进行中的流式对话；退出登录时一起取消。 */
    private static final ActiveRequests CHATS = new ActiveRequests(id -> cancel(id, true));
    /** 进行中的 AI 皮肤生成；退出登录时一起取消。 */
    private static final ActiveRequests SKINS = new ActiveRequests(id -> cancel(id, false));

    private RustAccount() {}

    /** 一次流式对话交出的一段增量（UTF-8 解码后），在调用 {@link #chatStream} 的工作线程上。 */
    public interface ChatListener {
        void onDelta(String text);
    }

    /** AI 皮肤每画完一张插画收到一次，completed 从 1 到 3，在调用 {@link #aiSkinGenerate} 的工作线程上。 */
    public interface SkinProgress {
        void onProgress(int completed);
    }

    /** 跑一个 {@code msime_client_account} 操作，返回它的 value。 */
    public static Object account(Context context, JSONObject action) throws AccountStore.Failure {
        return AccountStore.account(context, action);
    }

    /** {@link #account} 的 value 是对象时返回它，否则按 account_unavailable 失败。 */
    public static JSONObject accountObject(Context context, JSONObject action) throws AccountStore.Failure {
        Object value = account(context, action);
        if (!(value instanceof JSONObject object)) throw new AccountStore.Failure("account_unavailable", 0);
        return object;
    }

    /**
     * 登录账号优先、没有登录时用本机匿名账号的操作（反馈、诊断上传）。Rust 不会自己注册匿名账号：两种会话都没有（account_unauthorized）时先注册一次（受 Retry-After 约束，与 {@link BackendAnonymousAccount} 相同），再试一次。
     */
    public static Object preferredAccount(Context context, JSONObject action) throws AccountStore.Failure {
        try {
            return account(context, action);
        } catch (AccountStore.Failure failure) {
            if (!"account_unauthorized".equals(failure.code)) throw failure;
        }
        ensureAnonymous(context);
        return account(context, action);
    }

    /** 当前登录账号的编号；没有登录时为空串。只读 Rust 的本地状态，不联网、不刷新。 */
    public static String userId(Context context) throws AccountStore.Failure {
        Object value = account(context, operation("status"));
        JSONObject user = value instanceof JSONObject status ? status.optJSONObject("user") : null;
        return user == null ? "" : JsonPolicy.strictStringOrEmpty(user.opt("id"));
    }

    /** 登录的仍是 `expected`（空串表示「没有登录」）；不是时抛 account_cancelled，页面丢掉这次的结果。 */
    public static void requireUser(Context context, String expected) throws AccountStore.Failure {
        if (!userId(context).equals(expected == null ? "" : expected))
            throw new AccountStore.Failure("account_cancelled", 0);
    }

    /** 注册本机匿名账号（已有会话时只读一次存储）；服务端限流时是带 Retry-After 的 account_rate_limited。 */
    public static void ensureAnonymous(Context context) throws AccountStore.Failure {
        try {
            new BackendAnonymousAccount(context).ensureRegistered();
        } catch (AccountStore.Failure failure) {
            throw failure;
        } catch (BackendAnonymousAccount.RateLimited limited) {
            throw new AccountStore.Failure("account_rate_limited", (limited.retryAfterMillis() + 999) / 1000);
        } catch (Exception error) {
            throw new AccountStore.Failure("account_unavailable", 0);
        }
    }

    /**
     * 跑一个 {@code msime_client_community} 操作，返回它的 value。`stateRoot` 只有 keyboard_skin_install 这类要装进本机的操作需要，其余传 null。
     *
     * <p>下载键盘皮肤和举报在 Rust 里是登录账号优先、否则匿名账号；两种会话都没有（community_unauthorized）时，`anonymousFallback` 为真就先注册匿名账号再试一次。其余操作只认登录账号，传 false。
     */
    public static Object community(Context context, JSONObject action, String stateRoot, boolean anonymousFallback)
            throws AccountStore.Failure {
        try {
            return communityOnce(context, action, stateRoot);
        } catch (AccountStore.Failure failure) {
            if (!anonymousFallback || !"community_unauthorized".equals(failure.code)) throw failure;
        }
        ensureAnonymous(context);
        return communityOnce(context, action, stateRoot);
    }

    private static Object communityOnce(Context context, JSONObject action, String stateRoot)
            throws AccountStore.Failure {
        final String request;
        try {
            JSONObject body = new JSONObject().put("session_store", AccountStore.id(context)).put("action", action);
            if (stateRoot != null) body.put("state_root", stateRoot);
            request = body.toString();
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("community_invalid", 0);
        }
        return AccountStore.value(NativeClient.community(request));
    }

    /** 云剪贴板的账号路径（{@code msime_client_account_cloud_clipboard}）：这里只用不涉及内容的开关、保留时长、置顶、删除和清空；列表、上传和设备管理仍是 Java 的端到端加密驱动。 */
    public static Object cloudClipboard(Context context, JSONObject action) throws AccountStore.Failure {
        final String request;
        try {
            request = new JSONObject().put("session_store", AccountStore.id(context)).put("action", action).toString();
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("invalid_cloud_clipboard", 0);
        }
        return AccountStore.value(NativeClient.accountCloudClipboard(request));
    }

    /**
     * 流式对话（{@code msime_client_account_chat_stream}）：每到一段增量就交给 listener，返回完整回复。`model` 是 chat_models 的 defaultModel（Rust 要求必填）。请求进行中登记在 {@link #cancelChats} 里，退出登录时被取消，回来的是 account_cancelled。
     *
     * @param call 页面的取消把手；它的 {@link Call#cancel} 可以在任何线程上调用
     */
    public static String chatStream(Context context, JSONArray messages, String model, Call call,
            ChatListener listener) throws AccountStore.Failure {
        String id = CHATS.begin();
        try {
            if (!call.attach(id, true)) throw new AccountStore.Failure("account_cancelled", 0);
            final String request;
            try {
                request = new JSONObject().put("session_store", AccountStore.id(context)).put("request_id", id)
                    .put("messages", messages).put("model", model).toString();
            } catch (JSONException impossible) {
                throw new AccountStore.Failure("account_invalid", 0);
            }
            Object value = AccountStore.value(NativeClient.accountChatStream(request,
                text -> listener.onDelta(new String(text, StandardCharsets.UTF_8))));
            String content = value instanceof JSONObject reply ? JsonPolicy.strictStringOrEmpty(reply.opt("content")) : "";
            if (content.isEmpty()) throw new AccountStore.Failure("account_unavailable", 0);
            return content;
        } finally {
            call.detach(id);
            CHATS.end(id);
        }
    }

    /**
     * AI 皮肤一次生成三套（{@code msime_client_ai_skin_generate}），返回 Rust 的提案数组 `[{name,description,design,artworkPrompt,artwork}]`。登录账号优先、没有登录时用本机匿名账号，两种都没有时先注册匿名账号再试一次。
     *
     * <p>注册匿名账号失败时的 account_unavailable 换成 {@link AccountFailureText#AI_SKIN_ANONYMOUS_UNAVAILABLE}（{@link AccountFailureText#aiSkinAnonymousFailure}），页面只把 Rust 生成那一步的 account_unavailable 当作部署没开 AI 皮肤。
     */
    public static JSONArray aiSkinGenerate(Context context, String prompt, Call call, SkinProgress progress)
            throws AccountStore.Failure {
        try {
            return aiSkinOnce(context, prompt, call, progress);
        } catch (AccountStore.Failure failure) {
            if (!"account_unauthorized".equals(failure.code)) throw failure;
        }
        try {
            ensureAnonymous(context);
        } catch (AccountStore.Failure failure) {
            throw new AccountStore.Failure(AccountFailureText.aiSkinAnonymousFailure(failure.code),
                failure.retryAfterSeconds);
        }
        return aiSkinOnce(context, prompt, call, progress);
    }

    private static JSONArray aiSkinOnce(Context context, String prompt, Call call, SkinProgress progress)
            throws AccountStore.Failure {
        String id = SKINS.begin();
        try {
            if (!call.attach(id, false)) throw new AccountStore.Failure("ai_skin_cancelled", 0);
            final String request;
            try {
                request = new JSONObject().put("session_store", AccountStore.id(context)).put("request_id", id)
                    .put("prompt", prompt).toString();
            } catch (JSONException impossible) {
                throw new AccountStore.Failure("ai_skin_invalid", 0);
            }
            Object value = AccountStore.value(NativeClient.aiSkinGenerate(request, progress::onProgress));
            if (!(value instanceof JSONArray proposals)) throw new AccountStore.Failure("ai_skin_invalid", 0);
            return proposals;
        } finally {
            call.detach(id);
            SKINS.end(id);
        }
    }

    /** 退出登录时调：取消还在进行的流式对话和 AI 皮肤生成。 */
    public static void cancelForSignOut() {
        CHATS.cancelAll();
        SKINS.cancelAll();
    }

    private static void cancel(String id, boolean chat) {
        try {
            String request = new JSONObject().put("request_id", id).toString();
            if (chat) NativeClient.accountChatCancel(request);
            else NativeClient.aiSkinCancel(request);
        } catch (JSONException impossible) {
            // request_id 是 UUID 文本，拼不出 JSON 的情况不存在。
        }
    }

    /**
     * 页面的取消把手：页面关掉、点了停止时调 {@link #cancel}。还没开始时取消，开始那一刻就以取消结束；进行中取消，Rust 立即返回取消。
     */
    public static final class Call {
        private String id;
        private boolean chat;
        private boolean cancelled;

        public void cancel() {
            String running;
            boolean isChat;
            synchronized (this) {
                cancelled = true;
                running = id;
                isChat = chat;
            }
            if (running != null) RustAccount.cancel(running, isChat);
        }

        public synchronized boolean cancelled() { return cancelled; }

        synchronized boolean attach(String value, boolean isChat) {
            if (cancelled) return false;
            id = value;
            chat = isChat;
            return true;
        }

        synchronized void detach(String value) {
            if (value.equals(id)) id = null;
        }
    }

    private static JSONObject operation(String name) throws AccountStore.Failure {
        try {
            return new JSONObject().put("operation", name);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }
}
