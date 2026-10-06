package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * AI 设计皮肤的四类请求：`GET /v1/models` 取默认模型、`POST /v1/chat/completions` 让模型出三套设计、`POST /v1/skins/jobs` 为每套设计生成背景图并轮询 `GET /v1/skins/jobs/{id}`，最后 `DELETE` 释放任务。
 *
 * <p>提示词和答案的解析不在这里：系统提示词、请求体和「三套设计是否合格」都由 client-core 决定（{@link Planner}，默认经 `NativeClient.aiSkinPlan`），这里只管传输、轮询、取消与清理，和 client-core 的 `BackendAiSkinService::generate` 是同一条流程。身份用真实账号，没有时用设备的匿名账号；服务端按用户每天限额（默认 10 次），超出时 429 `rate_limit_exceeded` 加 `Retry-After`。503 表示这个部署没有开 AI 皮肤，页面据此隐藏入口。
 *
 * <p>纯 Java，不 import `androidx`、`R` 或 `home/`（check-host 会编译它）；全部方法阻塞，不要在主线程调用。
 */
public final class SkinJobsApi {
    /** 一次生成最多等多久背景图，与 client-core 的 `MAX_ARTWORK_SECONDS` 一致。 */
    static final long ARTWORK_TIMEOUT_MILLIS = 200_000;
    /** 任务还在运行时两次查询之间等多久。 */
    static final long POLL_INTERVAL_MILLIS = 5_000;
    /** 描述的字符上限，与 client-core 一致。 */
    public static final int MAX_PROMPT_CHARACTERS = 500;
    /** 一次生成的设计套数；页面预览和背景图任务并行度共用这个上限。 */
    public static final int MAX_DESIGNS = 3;
    private static final Pattern JOB_ID = Pattern.compile("[0-9a-f]{1,48}");

    /** 模型给出的一套设计和它的背景图。`design` 是 `custom_theme.keyboard` 的形式（不含照片），`artwork` 是校验过的图片。 */
    public record Proposal(String name, String description, JSONObject design, Artwork artwork) {}

    /** 一张背景图：base64 编码的 PNG 或 JPEG 与它的像素尺寸。 */
    public record Artwork(String base64, String mimeType, int width, int height) {}

    /** 一个生成背景图的任务；`artwork` 只在成功时有。 */
    public record Job(String id, String state, Artwork artwork) {}

    /** client-core 那一侧的三个决定；冒烟测试换成内存实现。 */
    public interface Planner {
        /** 拼出聊天请求：返回 `{path, body}`。 */
        JSONObject compose(String prompt, String model) throws CloudApi.Failure;

        /** 把模型的回答读成三套设计 `[{name, description, artworkPrompt, design}]`；不合格时抛出。 */
        JSONArray parse(String text) throws CloudApi.Failure;

        /** 这张图是否是本客户端会显示的图片。 */
        boolean artworkValid(JSONObject artwork);
    }

    /** 经 JNI 调 client-core 的 {@link Planner}。 */
    public static final Planner NATIVE = new Planner() {
        @Override public JSONObject compose(String prompt, String model) throws CloudApi.Failure {
            try {
                return value(NativeClient.aiSkinPlan(new JSONObject().put("operation", "compose")
                    .put("prompt", prompt).put("model", model).toString()));
            } catch (JSONException error) {
                throw invalid("ai_skin_invalid");
            }
        }

        @Override public JSONArray parse(String text) throws CloudApi.Failure {
            final String response;
            try {
                response = NativeClient.aiSkinPlan(new JSONObject().put("operation", "parse")
                    .put("text", text).toString());
            } catch (JSONException error) {
                throw invalid("ai_skin_response");
            }
            try {
                JSONObject root = new JSONObject(response == null ? "" : response);
                JSONArray plans = Boolean.TRUE.equals(strictBoolean(root.opt("ok")))
                    ? root.optJSONArray("value") : null;
                if (plans == null) throw invalid("ai_skin_response");
                return plans;
            } catch (JSONException error) {
                throw invalid("ai_skin_response");
            }
        }

        @Override public boolean artworkValid(JSONObject artwork) {
            try {
                JSONObject root = new JSONObject(NativeClient.aiSkinPlan(new JSONObject()
                    .put("operation", "artwork").put("artwork", artwork).toString()));
                return Boolean.TRUE.equals(strictBoolean(root.opt("ok")));
            } catch (JSONException | RuntimeException error) {
                return false;
            }
        }
    };

    /** 用来等待的钟；冒烟测试换成不真睡的实现。 */
    public interface Clock {
        long now();

        void sleep(long millis) throws InterruptedException;
    }

    static final Clock SYSTEM_CLOCK = new Clock() {
        @Override public long now() { return System.currentTimeMillis(); }

        @Override public void sleep(long millis) throws InterruptedException { Thread.sleep(millis); }
    };

    private final CloudApi api;
    private final Planner planner;
    private final Clock clock;

    public SkinJobsApi(CloudApi api) {
        this(api, NATIVE, SYSTEM_CLOCK);
    }

    public SkinJobsApi(CloudApi api, Planner planner, Clock clock) {
        this.api = api;
        this.planner = planner;
        this.clock = clock;
    }

    /** 服务端今天的生成次数已用完（429 `rate_limit_exceeded`）。 */
    public static boolean quotaExhausted(CloudApi.Failure failure) {
        return failure.status == 429;
    }

    /** 这个部署没有开 AI 皮肤（任何 503），入口应当隐藏。 */
    public static boolean unavailable(CloudApi.Failure failure) {
        return failure.status == 503;
    }

    /** A planner response must stay within the number of designs this flow can own and display. */
    public static boolean validPlanCount(int count) {
        return count >= 1 && count <= MAX_DESIGNS;
    }

    /** 失败给用户看的一句话。 */
    public static String message(CloudApi.Failure failure) {
        if (quotaExhausted(failure)) {
            long hours = (failure.retryAfterSeconds + 3599) / 3600;
            return hours > 0 ? "今天的生成次数已用完，约 " + hours + " 小时后可以再试" : "今天的生成次数已用完";
        }
        if (unavailable(failure)) return "AI 设计皮肤暂不可用";
        // 本地失败的状态也是 0，先按错误码分，再把剩下的 0 当作网络问题。
        switch (failure.code) {
            case "cancelled": return "已取消";
            case "ai_skin_response": return "这次的设计不合格，换个描述再试一次";
            case "ai_skin_invalid": return "描述不能为空，最多 " + MAX_PROMPT_CHARACTERS + " 个字";
            case "ai_skin_timeout": return "背景图生成超时，请稍后重试";
            case "ai_skin_unavailable": case "invalid_response": return "生成失败，请稍后重试";
            default: break;
        }
        if (failure.signedOut()) return "请先登录后再生成";
        if (failure.network()) return "连不上服务器，请检查网络后重试";
        return "生成失败，请稍后重试";
    }

    /**
     * 按描述生成三套设计并为每套取回背景图。三张图并行生成；任何一张失败或 `cancelled` 被置位时，其余的一并取消，已经创建的任务都会被删除。
     *
     * @param cancelled 置为 true 即取消，调用方在页面离开时置位
     */
    public List<Proposal> generate(String prompt, AtomicBoolean cancelled) throws CloudApi.Failure {
        String trimmed = prompt == null ? "" : prompt.trim();
        if (trimmed.isEmpty() || trimmed.codePointCount(0, trimmed.length()) > MAX_PROMPT_CHARACTERS)
            throw invalid("ai_skin_invalid");
        check(cancelled);
        String model = defaultModel();
        check(cancelled);
        String answer = chat(planner.compose(trimmed, model));
        JSONArray plans = planner.parse(answer);
        if (plans == null || !validPlanCount(plans.length())) throw invalid("ai_skin_response");
        check(cancelled);

        ExecutorService pool = Executors.newFixedThreadPool(BoundsPolicy.atMost(plans.length(), MAX_DESIGNS), runnable -> {
            Thread thread = new Thread(runnable, "msime-ai-skin");
            thread.setDaemon(true);
            return thread;
        });
        try {
            List<Future<Proposal>> futures = new ArrayList<>(plans.length());
            for (int index = 0; index < plans.length(); index++) {
                JSONObject plan = plans.optJSONObject(index);
                if (plan == null) throw invalid("ai_skin_response");
                futures.add(pool.submit(() -> illustrate(plan, cancelled)));
            }
            List<Proposal> proposals = new ArrayList<>(futures.size());
            CloudApi.Failure first = null;
            for (Future<Proposal> future : futures) {
                try {
                    proposals.add(future.get());
                } catch (ExecutionException error) {
                    cancelled.set(true);
                    if (first == null) {
                        first = error.getCause() instanceof CloudApi.Failure failure ? failure
                            : new CloudApi.Failure(0, "ai_skin_unavailable", String.valueOf(error.getCause()), 0);
                    }
                } catch (InterruptedException error) {
                    cancelled.set(true);
                    Thread.currentThread().interrupt();
                    if (first == null) first = new CloudApi.Failure(0, "cancelled", "interrupted", 0);
                }
            }
            if (first != null) throw first;
            return List.copyOf(proposals);
        } finally {
            pool.shutdownNow();
        }
    }

    /** `GET /v1/models` 的 `default_model`；它必须出现在 `data` 里。 */
    public String defaultModel() throws CloudApi.Failure {
        JSONObject catalog = api.json("GET", "/v1/models", null, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        Object raw = catalog.opt("default_model");
        JSONArray data = catalog.optJSONArray("data");
        if (!(raw instanceof String model) || model.isEmpty() || model.length() > 200 || data == null)
            throw invalid("invalid_response");
        for (int index = 0; index < data.length(); index++) {
            JSONObject entry = data.optJSONObject(index);
            if (entry != null && model.equals(entry.opt("id"))) return model;
        }
        throw invalid("invalid_response");
    }

    /** 发 client-core 拼好的聊天请求，返回助手回答的正文。 */
    public String chat(JSONObject composed) throws CloudApi.Failure {
        String path = composed.optString("path", "");
        JSONObject body = composed.optJSONObject("body");
        if (!"/v1/chat/completions".equals(path) || body == null) throw invalid("ai_skin_invalid");
        JSONObject result = api.json("POST", path, body, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        JSONArray choices = result.optJSONArray("choices");
        JSONObject first = choices == null ? null : choices.optJSONObject(0);
        JSONObject message = first == null ? null : first.optJSONObject("message");
        Object content = message == null ? null : message.opt("content");
        if (message == null || !"assistant".equals(message.opt("role")) || !(content instanceof String text)
                || text.trim().isEmpty() || text.length() > 16 * 1024)
            throw invalid("ai_skin_response");
        return text;
    }

    /** `POST /v1/skins/jobs`：为一段背景描述开一个生成任务。 */
    public Job create(String artworkPrompt) throws CloudApi.Failure {
        try {
            return job(api.json("POST", "/v1/skins/jobs", new JSONObject().put("prompt", artworkPrompt),
                CloudApi.Auth.ACCOUNT_OR_ANONYMOUS));
        } catch (JSONException error) {
            throw invalid("ai_skin_invalid");
        }
    }

    /** `GET /v1/skins/jobs/{id}`。 */
    public Job get(String id) throws CloudApi.Failure {
        requireId(id);
        Job job = job(api.json("GET", "/v1/skins/jobs/" + id, null, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS));
        if (!job.id().equals(id)) throw invalid("invalid_response");
        return job;
    }

    /** `DELETE /v1/skins/jobs/{id}`：成功、失败和取消的任务都要释放，免得上游任务继续计费。 */
    public void delete(String id) throws CloudApi.Failure {
        requireId(id);
        api.send("DELETE", "/v1/skins/jobs/" + id, null, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
    }

    private Proposal illustrate(JSONObject plan, AtomicBoolean cancelled) throws CloudApi.Failure {
        check(cancelled);
        Object name = plan.opt("name");
        Object description = plan.opt("description");
        Object artworkPrompt = plan.opt("artworkPrompt");
        JSONObject design = plan.optJSONObject("design");
        if (!(name instanceof String title) || !(description instanceof String detail)
                || !(artworkPrompt instanceof String scene) || design == null)
            throw invalid("ai_skin_response");
        Job job = create(scene);
        try {
            return new Proposal(title, detail, design, poll(job, cancelled));
        } finally {
            try {
                delete(job.id());
            } catch (CloudApi.Failure ignored) {
                // 释放失败不影响这次结果：服务端会按过期时间清理任务。
            }
        }
    }

    private Artwork poll(Job started, AtomicBoolean cancelled) throws CloudApi.Failure {
        long deadline = clock.now() + ARTWORK_TIMEOUT_MILLIS;
        Job job = started;
        while (true) {
            check(cancelled);
            switch (job.state()) {
                case "succeeded" -> {
                    if (job.artwork() == null) throw invalid("ai_skin_response");
                    return job.artwork();
                }
                case "running" -> {
                    if (clock.now() >= deadline)
                        throw new CloudApi.Failure(0, "ai_skin_timeout", "artwork timed out", 0);
                    long until = clock.now() + POLL_INTERVAL_MILLIS;
                    while (clock.now() < until) {
                        check(cancelled);
                        try {
                            clock.sleep(100);
                        } catch (InterruptedException error) {
                            Thread.currentThread().interrupt();
                            throw new CloudApi.Failure(0, "cancelled", "interrupted", 0);
                        }
                    }
                    job = get(job.id());
                }
                default -> throw new CloudApi.Failure(0, "ai_skin_unavailable", "artwork failed", 0);
            }
        }
    }

    private Job job(JSONObject value) throws CloudApi.Failure {
        Object id = value.opt("id");
        Object state = value.opt("state");
        if (!(id instanceof String identifier) || !JOB_ID.matcher(identifier).matches()
                || !(state instanceof String status)
                || !("running".equals(status) || "succeeded".equals(status) || "failed".equals(status)))
            throw invalid("invalid_response");
        JSONObject raw = value.optJSONObject("artwork");
        Artwork artwork = null;
        if (raw != null) {
            Object data = raw.opt("b64_json");
            Object mime = raw.opt("mime_type");
            Integer width = strictArtworkDimension(raw.opt("width"));
            Integer height = strictArtworkDimension(raw.opt("height"));
            if (!(data instanceof String base64) || !(mime instanceof String type)
                    || !("image/png".equals(type) || "image/jpeg".equals(type))
                    || width == null || height == null
                    || base64.length() > 11 * 1024 * 1024 || !planner.artworkValid(raw))
                throw invalid("ai_skin_response");
            artwork = new Artwork(base64, type, width, height);
        }
        return new Job(identifier, status, artwork);
    }

    private static void requireId(String id) {
        if (id == null || !JOB_ID.matcher(id).matches()) throw new IllegalArgumentException("invalid job id");
    }

    private static void check(AtomicBoolean cancelled) throws CloudApi.Failure {
        if (cancelled.get()) throw new CloudApi.Failure(0, "cancelled", "cancelled", 0);
    }

    private static CloudApi.Failure invalid(String code) {
        return new CloudApi.Failure(0, code, code, 0);
    }

    /** Artwork dimensions must be JSON integers in the decoded image bounds. */
    public static Integer strictArtworkDimension(Object value) {
        if (value instanceof Integer integer && integer >= 1 && integer <= 2048) return integer;
        if (value instanceof Long longValue && longValue >= 1L && longValue <= 2048L) {
            return longValue.intValue();
        }
        return null;
    }

    /** Native planner responses must keep status flags as JSON booleans. */
    static Boolean strictBoolean(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
    }

    private static JSONObject value(String response) throws CloudApi.Failure {
        try {
            JSONObject root = new JSONObject(response == null ? "" : response);
            JSONObject value = Boolean.TRUE.equals(strictBoolean(root.opt("ok")))
                ? root.optJSONObject("value") : null;
            if (value == null) throw invalid("ai_skin_invalid");
            return value;
        } catch (JSONException error) {
            throw invalid("ai_skin_invalid");
        }
    }
}
