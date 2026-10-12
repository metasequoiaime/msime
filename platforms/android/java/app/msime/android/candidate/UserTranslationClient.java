package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 用用户自己选的服务（小牛、自定义、腾讯）给候选词翻译，写法照 iOS 的 {@code TranslationProviderClient}。每个请求都由共享层构造（签名、鉴权头和 DeepL 与 DeepLX 的格式差别都在 Rust 里），每个回复都由共享层解析，这里只决定发哪些词、用 {@link TranslationHttpTransport} 搬字节。
 *
 * <ul>
 * <li>只翻译不超过 {@link #MAX_SOURCE_CODE_POINTS} 个字的词，按码点计，与共享层按 Unicode 标量计数一致；更长的词不发。</li>
 * <li>腾讯一次最多 {@link #TENCENT_BATCH} 个词，带秒级时间戳；小牛和自定义一词一请求，小牛带毫秒时间戳。源语言固定是中文，目标语言是 store 传进来的代码。</li>
 * <li>逐词（逐批）之间问一次 store 的 stale，用户接着打字后立刻停下，不把过期的一页发完。</li>
 * <li>问过的词连同回答（包括「没有译文」）记在 {@link UserTranslationMemory} 里，再问时直接用记下的，不联网；失败后的退避也在那里。</li>
 * </ul>
 *
 * <p>路由不可变，换服务或换凭据时键盘新建一个客户端，旧客户端的负缓存随之丢掉。描述器和回复带着凭据和候选词，任何地方都不记录它们。
 */
public final class UserTranslationClient implements CandidateTranslationStore.Service {
    /** 腾讯每批的词数，与 iOS、Linux 相同（共享层上限是 10）。 */
    public static final int TENCENT_BATCH = 9;
    /** 共享层 `MAX_SOURCE_CHARS`：超过的词描述器会拒绝，所以根本不发。 */
    public static final int MAX_SOURCE_CODE_POINTS = 40;
    /** store 拿到的译文超过这个字节数会丢掉，所以也不算回答。 */
    private static final int MAX_GLOSS_BYTES = 4096;

    private final CandidateTranslationRoute route;
    private final UserTranslationMemory memory;

    public UserTranslationClient(CandidateTranslationRoute route) {
        this(route, new UserTranslationMemory(() -> TimeUnit.NANOSECONDS.toMillis(System.nanoTime())));
    }

    UserTranslationClient(CandidateTranslationRoute route, UserTranslationMemory memory) {
        this.route = route;
        this.memory = memory;
    }

    /** 这个客户端绑定的路由；键盘拿它判断偏好重读后能不能沿用这个客户端和它记下的回答。 */
    public CandidateTranslationRoute route() {
        return route;
    }

    @Override public List<String> translate(List<String> texts, String target) throws Exception {
        return translate(texts, target, () -> false);
    }

    @Override public List<String> translate(List<String> texts, String target, BooleanSupplier stale)
            throws Exception {
        ArrayList<String> result = new ArrayList<>(Collections.nCopies(texts.size(), (String) null));
        String language = TextPolicy.lowercaseTrimmed(target);
        if (!route.userService() || language.isEmpty()) return result;
        ArrayList<Integer> eligible = new ArrayList<>(texts.size());
        for (int index = 0; index < texts.size(); index++) {
            String word = texts.get(index);
            if (word == null || word.isEmpty()
                    || word.codePointCount(0, word.length()) > MAX_SOURCE_CODE_POINTS) continue;
            String known = memory.answer(language, word);
            if (known != null) {
                result.set(index, known.isEmpty() ? null : known);
                continue;
            }
            if (memory.shouldAsk(language, word)) eligible.add(index);
        }
        if (route.provider() == CandidateTranslationRoute.Provider.TENCENT) {
            for (int start = 0; start < eligible.size(); start += TENCENT_BATCH) {
                if (stale.getAsBoolean() || memory.serviceBackingOff()) break;
                tencent(texts, eligible.subList(start, Math.min(eligible.size(), start + TENCENT_BATCH)),
                    language, result);
            }
            return result;
        }
        for (int index : eligible) {
            if (stale.getAsBoolean() || memory.serviceBackingOff()) break;
            String word = texts.get(index);
            result.set(index, route.provider() == CandidateTranslationRoute.Provider.NIUTRANS
                ? niutrans(word, language) : custom(word, language));
        }
        return result;
    }

    private String niutrans(String word, String language) throws JSONException {
        JSONObject request = new JSONObject()
            .put("config", new JSONObject().put("enabled", true)
                .put("app_id", route.appId()).put("apikey", route.apiKey()))
            .put("text", word).put("source_language", "zh").put("target_language", language)
            .put("timestamp", Long.toString(System.currentTimeMillis()));
        JSONObject descriptor = descriptor(NativeClient.niutransTranslationHttpRequest(request.toString()));
        if (descriptor == null) return null;
        byte[] body = TranslationHttpTransport.post(descriptor);
        if (body == null) {
            memory.unreachable();
            return null;
        }
        if (NativeClient.niutransTranslationReplyFailed(body)) {
            memory.failed(language, word);
            return null;
        }
        return answer(language, word, stringValue(NativeClient.parseNiuTransTranslationResponse(body)));
    }

    private String custom(String word, String language) throws JSONException {
        JSONObject request = new JSONObject()
            .put("config", new JSONObject().put("enabled", true)
                .put("endpoint", route.endpoint()).put("api_key", route.apiKey()))
            .put("text", word).put("source_language", "zh").put("target_language", language);
        // DeepL 地址没填密钥之类，共享层构造不出请求：这个词不联网，也就谈不上额度，不记退避。
        JSONObject descriptor = descriptor(NativeClient.customTranslationHttpRequest(request.toString()));
        if (descriptor == null) return null;
        byte[] body = TranslationHttpTransport.post(descriptor);
        if (body == null) {
            memory.unreachable();
            return null;
        }
        if (NativeClient.customTranslationReplyFailed(body)) {
            memory.failed(language, word);
            return null;
        }
        return answer(language, word, stringValue(NativeClient.parseCustomTranslationResponse(body)));
    }

    private void tencent(List<String> texts, List<Integer> batch, String language, List<String> result)
            throws JSONException {
        JSONArray words = new JSONArray();
        for (int index : batch) words.put(texts.get(index));
        JSONObject request = new JSONObject()
            .put("config", new JSONObject().put("enabled", true).put("secret_id", route.secretId())
                .put("secret_key", route.secretKey()).put("region", route.region()))
            .put("texts", words).put("source_language", "zh").put("target_language", language)
            .put("timestamp", TimeUnit.MILLISECONDS.toSeconds(System.currentTimeMillis()));
        JSONObject descriptor = descriptor(NativeClient.tencentTranslationHttpRequest(request.toString()));
        if (descriptor == null) return;
        byte[] body = TranslationHttpTransport.post(descriptor);
        if (body == null) {
            memory.unreachable();
            return;
        }
        // 共享解析器对报错的回复（凭据、额度、限流）整个返回 null；这些都是整个账号的问题，按服务不可达退避。
        JSONArray glosses = arrayValue(NativeClient.parseTencentTranslationResponse(body, batch.size()));
        if (glosses == null || glosses.length() != batch.size()) {
            memory.unreachable();
            return;
        }
        for (int position = 0; position < batch.size(); position++) {
            int index = batch.get(position);
            result.set(index, answer(language, texts.get(index), JsonPolicy.strictString(glosses.opt(position))));
        }
    }

    /** 记下服务给出的回答：没有能显示的译文（空、与原词相同、超长）也是回答，以后同一服务下不再问这个词。 */
    private String answer(String language, String word, String gloss) {
        String shown = TextPolicy.stripSpaceChars(gloss);
        boolean usable = shown != null && !shown.isEmpty() && !shown.equals(word)
            && TextPolicy.utf8Length(shown) <= MAX_GLOSS_BYTES;
        memory.answered(language, word, usable ? shown : null);
        return usable ? shown : null;
    }

    /** 共享层回复信封里的描述器；构造失败或服务关闭时为 null。 */
    private static JSONObject descriptor(String envelope) throws JSONException {
        Object value = envelopeValue(envelope);
        return value instanceof JSONObject ? (JSONObject) value : null;
    }

    private static String stringValue(String envelope) throws JSONException {
        return JsonPolicy.strictString(envelopeValue(envelope));
    }

    private static JSONArray arrayValue(String envelope) throws JSONException {
        Object value = envelopeValue(envelope);
        return value instanceof JSONArray ? (JSONArray) value : null;
    }

    private static Object envelopeValue(String envelope) throws JSONException {
        if (envelope == null || envelope.isEmpty()) return null;
        JSONObject root = new JSONObject(envelope);
        return JsonPolicy.strictTrue(root.opt("ok")) ? root.opt("value") : null;
    }
}
