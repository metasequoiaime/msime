package app.msime.android;

import android.content.Context;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * AI 设计皮肤：由 Rust 的 {@code msime_client_ai_skin_generate} 一次生成三套带插画的方案（#8201 迁移计划 5c）。默认模型、拼提示词、校验模型给出的方案、为每套开插画任务并轮询、出错或取消时删掉已开的任务，都在 Rust 里，与 Tauri 的 ai_skin_generate 是同一条流程。
 *
 * <p>登录账号优先，没有登录时用设备的匿名账号；一次生成从头到尾用开始时选定的那个账号。服务端按用户每天限额（默认 10 次），超出时是带 Retry-After 的 account_rate_limited；没开 AI 皮肤的部署是不带 Retry-After 的 account_unavailable，页面据此隐藏入口（{@link AccountFailureText#aiSkinUnavailable}）。
 *
 * <p>纯 Java，不 import `androidx`、`R` 或 `home/`（check-host 会编译它）；生成阻塞约几分钟，不要在主线程调用。
 */
public final class SkinJobsApi {
    /** 描述的字符上限，与 Rust 一致（1–500 个字符、不含控制字符）。 */
    public static final int MAX_PROMPT_CHARACTERS = 500;
    /** 一次生成的设计套数；Rust 总是恰好给三套。 */
    public static final int MAX_DESIGNS = 3;

    /** 模型给出的一套设计和它的背景图。`design` 是 `custom_theme.keyboard` 的形式（不含照片），`artwork` 是 Rust 校验过的图片。 */
    public record Proposal(String name, String description, JSONObject design, Artwork artwork) {}

    /** 一张背景图：base64 编码的 PNG 或 JPEG 与它的像素尺寸。 */
    public record Artwork(String base64, String mimeType, int width, int height) {}

    private SkinJobsApi() {}

    /** 描述可以交给 Rust：去掉首尾空白后 1–500 个字符，不含控制字符。 */
    public static boolean validPrompt(String prompt) {
        String trimmed = TextPolicy.trimmed(prompt);
        return !trimmed.isEmpty() && TextPolicy.withinCodePoints(trimmed, MAX_PROMPT_CHARACTERS)
            && !TextPolicy.hasControl(trimmed);
    }

    /**
     * 按描述生成三套设计和它们的背景图。`call` 是页面的取消把手：页面真的关掉时调它的 cancel，Rust 随即以 ai_skin_cancelled 结束，已经开出的插画任务在返回前删掉。
     */
    public static List<Proposal> generate(Context context, String prompt, RustAccount.Call call)
            throws AccountStore.Failure {
        if (!validPrompt(prompt)) throw new AccountStore.Failure("ai_skin_invalid", 0);
        JSONArray values = RustAccount.aiSkinGenerate(context, TextPolicy.trimmed(prompt), call, completed -> { });
        if (values.length() == 0 || values.length() > MAX_DESIGNS) throw new AccountStore.Failure("ai_skin_invalid", 0);
        List<Proposal> proposals = new ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            JSONObject value = values.optJSONObject(index);
            Proposal proposal = value == null ? null : proposal(value);
            if (proposal == null) throw new AccountStore.Failure("ai_skin_invalid", 0);
            proposals.add(proposal);
        }
        return ListPolicy.copyOrEmpty(proposals);
    }

    /** Rust 的一套提案 `{name,description,design,artworkPrompt,artwork:{b64_json,mime_type,width,height}}`；形状不对时为 null。 */
    private static Proposal proposal(JSONObject value) {
        String name = JsonPolicy.strictString(value.opt("name"));
        String description = JsonPolicy.strictString(value.opt("description"));
        JSONObject design = value.optJSONObject("design");
        JSONObject artwork = value.optJSONObject("artwork");
        if (name == null || description == null || design == null || artwork == null) return null;
        String base64 = JsonPolicy.strictString(artwork.opt("b64_json"));
        String mime = JsonPolicy.strictString(artwork.opt("mime_type"));
        Integer width = strictArtworkDimension(artwork.opt("width"));
        Integer height = strictArtworkDimension(artwork.opt("height"));
        if (base64 == null || width == null || height == null
                || !("image/png".equals(mime) || "image/jpeg".equals(mime))) return null;
        return new Proposal(name, description, design, new Artwork(base64, mime, width, height));
    }

    /** Artwork dimensions must be JSON integers in the decoded image bounds. */
    public static Integer strictArtworkDimension(Object value) {
        if (value instanceof Integer integer && integer >= 1 && integer <= 2048) return integer;
        if (value instanceof Long longValue && longValue >= 1L && longValue <= 2048L) {
            return longValue.intValue();
        }
        return null;
    }
}
