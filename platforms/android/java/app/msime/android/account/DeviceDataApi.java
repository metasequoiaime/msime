package app.msime.android;

import android.content.Context;
import java.io.File;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashSet;
import java.util.List;
import java.util.UUID;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「我的」里个人资料、我的设备与云端数据：资料、改名、头像、退出与注销、会话列表与撤销、云端数据汇总 / 导出 / 删除，都经 Rust 的 {@code msime_client_account}（#8201 迁移计划 5c，见 {@link RustAccount}）。
 *
 * <p>只用登录账号（匿名账号不算「我的」）。一个实例绑定页面打开时的账号编号（user.id）：每次请求前后都核对登录的仍是它，退出或换了号就以 account_cancelled 失败，页面丢掉结果。注销账号和删除云端数据要求 10 分钟内登录过，Rust 报 account_recent_login_required 时这里抛 {@link RecentLoginRequired}，界面据此让用户重新登录一次再重试。
 *
 * <p>每个方法都阻塞（读状态也要等 Rust 的会话锁），不要在主线程调用。
 */
public final class DeviceDataApi {
    /** 头像上传上限，与服务端 `maxAvatarUploadBytes` 一致。 */
    public static final int MAX_AVATAR_BYTES = 1024 * 1024;
    /** 云端数据里可以单独删除的分类，顺序即确认框里的顺序；voice 是这台设备匿名账号名下的语音样本（Rust 的 data_delete 另删匿名账号的那一份）。 */
    public static final List<String> DELETABLE_SECTIONS =
        List.of("preferences", "dictionary", "phrases", "clipboard", "voice");

    /** 这个操作要求 10 分钟内登录过；重新登录同一个账号后再试。 */
    public static final class RecentLoginRequired extends Exception {
        private static final long serialVersionUID = 1L;

        public RecentLoginRequired(AccountStore.Failure cause) {
            super("recent login required", cause);
        }
    }

    /** Rust profile 的用户与已关联的登录方式（去重后的 provider 名）。`hasAvatar` 是 Rust 给了本客户端会去取的头像地址，这时才值得调 {@link #avatar}。 */
    public record Profile(String id, String displayName, String email, boolean hasAvatar, List<String> providers) {
        public boolean linked(String provider) { return providers.contains(provider); }

        /** 真实账号的登录方式里排在最前的一种（google / apple / email），一种都没有时为空字符串。 */
        public String loginKind() {
            for (String kind : SyncSwitch.LOGIN_KINDS) if (providers.contains(kind)) return kind;
            return "";
        }
    }

    /** 一个未撤销、未过期的登录会话；时间是 Unix 毫秒，读不出时为 0。 */
    public record Session(String id, String platform, String name, String appVersion, long createdAt,
            long lastActive, boolean current) {}

    /** 云端数据的一个分类。 */
    public record DataSection(String id, long bytes, long items) {}

    /**
     * 云端数据汇总：总字节数（统计口径，不是精确的磁盘占用）与各分类。`anonymousVoice` 是 Rust 合并本机匿名账号语音样本的结果：included、none，或 unknown（这一次没合进来，voice 只是登录账号的，页面写「语音样本数暂不可知」）。
     */
    public record DataSummary(long bytes, List<DataSection> sections, String anonymousVoice) {}

    /** 注销账号或删除云端数据之后，删本机匿名账号语音样本那一步的结果；没有删到语音样本这一步时 ok 为 true。 */
    public record VoiceSamples(boolean ok, String code, long retryAfterSeconds) {
        static final VoiceSamples DONE = new VoiceSamples(true, "", 0);
    }

    private final Context application;
    private final String userId;

    /** 绑定页面打开时的账号编号；空串表示没有登录，这时每个操作都以 account_unauthorized 失败。 */
    public DeviceDataApi(Context context, String expectedUserId) {
        this.application = context.getApplicationContext();
        this.userId = expectedUserId == null ? "" : expectedUserId;
    }

    /** 当前登录账号的编号，没有登录时为空串。不联网，但要在工作线程上调用。 */
    public static String currentUserId(Context context) throws AccountStore.Failure {
        return RustAccount.userId(context);
    }

    // ---- 个人资料 ----

    public Profile profile() throws AccountStore.Failure {
        return parseProfile(call(op("profile")));
    }

    /** 头像的图片字节（PNG、JPEG 或 WebP）；没有头像时为 null。图片由 Rust 下载，只认账号头像服务的地址。 */
    public byte[] avatar() throws AccountStore.Failure {
        Object value = bound(() -> RustAccount.account(application, op("avatar")));
        if (!(value instanceof String url)) return null;
        return avatarBytes(url);
    }

    /** 改昵称：规范化和合法性都由共享 Rust 计算。 */
    public Profile rename(String displayName) throws AccountStore.Failure {
        AccountNamePolicy.Presentation name = AccountNamePolicy.present(displayName);
        if (name == null || !name.valid()) throw new AccountStore.Failure("account_invalid", 0);
        try {
            return parseProfile(call(op("rename").put("display_name", name.normalized())));
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    /** 上传头像：`image` 是已经核对过的 PNG 或 JPEG（≤1 MiB），Rust 读这个私有文件后上传。 */
    public Profile uploadAvatar(File image) throws AccountStore.Failure {
        try {
            return parseProfile(call(op("upload_avatar").put("path", image.getAbsolutePath())));
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    /** 退出登录并撤销服务端的这次登录（`logout{all:false}`）。Rust 先清本地会话再请求服务端：失败时本机已经退出，失败照样抛出，页面说明服务端没有撤销。 */
    public void logout() throws AccountStore.Failure {
        RustAccount.requireUser(application, userId);
        try {
            RustAccount.account(application, op("logout").put("all", false));
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    /** 注销账号：服务端立即删除账号与它的全部云端数据、社区作品和会话，Rust 随后清掉本地会话，再尽力删本机匿名账号名下的语音样本。要求最近登录。 */
    public VoiceSamples deleteAccount() throws AccountStore.Failure, RecentLoginRequired {
        RustAccount.requireUser(application, userId);
        final Object value;
        try {
            value = RustAccount.account(application, op("account_delete"));
        } catch (AccountStore.Failure failure) {
            throw recentLogin(failure);
        }
        return voiceSamples(value);
    }

    // ---- 我的设备 ----

    public List<Session> sessions() throws AccountStore.Failure {
        JSONArray rows = call(op("sessions_list")).optJSONArray("sessions");
        if (rows == null) throw new AccountStore.Failure("account_unavailable", 0);
        List<Session> sessions = new ArrayList<>(rows.length());
        for (int index = 0; index < rows.length(); index++) {
            JSONObject row = rows.optJSONObject(index);
            if (row == null) continue;
            sessions.add(new Session(JsonPolicy.strictStringOrEmpty(row.opt("id")),
                JsonPolicy.strictStringOrEmpty(row.opt("platform")),
                JsonPolicy.strictStringOrEmpty(row.opt("name")),
                JsonPolicy.strictStringOrEmpty(row.opt("app_version")),
                instant(JsonPolicy.strictStringOrEmpty(row.opt("created_at"))),
                instant(JsonPolicy.strictStringOrEmpty(row.opt("last_active"))),
                JsonPolicy.strictTrue(row.opt("current"))));
        }
        return ListPolicy.copyOrEmpty(sessions);
    }

    /** 撤销一个会话；撤销的是本机这一次时 Rust 当作退出登录、清掉本地会话，返回 true，调用方随后清本机的同步状态。 */
    public boolean revokeSession(String id) throws AccountStore.Failure {
        RustAccount.requireUser(application, userId);
        final Object value;
        try {
            value = RustAccount.account(application, op("session_revoke").put("id", id));
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        return value instanceof JSONObject result && JsonPolicy.strictTrue(result.opt("signed_out"));
    }

    // ---- 云端数据 ----

    public DataSummary dataSummary() throws AccountStore.Failure {
        JSONObject root = call(op("data_summary"));
        JSONArray rows = root.optJSONArray("sections");
        if (rows == null) throw new AccountStore.Failure("account_unavailable", 0);
        List<DataSection> sections = new ArrayList<>(rows.length());
        for (int index = 0; index < rows.length(); index++) {
            JSONObject row = rows.optJSONObject(index);
            if (row == null || JsonPolicy.strictStringOrEmpty(row.opt("id")).isEmpty()) continue;
            sections.add(new DataSection(JsonPolicy.strictStringOrEmpty(row.opt("id")),
                JsonPolicy.nonNegativeLong(row.opt("bytes"), 0), JsonPolicy.nonNegativeLong(row.opt("items"), 0)));
        }
        return new DataSummary(JsonPolicy.nonNegativeLong(root.opt("bytes"), 0), ListPolicy.copyOrEmpty(sections),
            JsonPolicy.strictStringOrEmpty(root.opt("anonymous_voice")));
    }

    /**
     * 把一次性导出的 zip 写到 `destination`（每用户每天 3 次，超出时是带 Retry-After 的 account_rate_limited）。Rust 先写进同目录的临时文件，读得出 zip 的中央目录才换到 destination；失败时 destination 上原有的文件不动。
     *
     * @return 写入的字节数
     */
    public long exportData(File destination) throws AccountStore.Failure {
        try {
            JSONObject result = call(op("data_export").put("destination", destination.getAbsolutePath()));
            return JsonPolicy.nonNegativeLong(result.opt("bytes"), 0);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    /** 只删数据、保留账号：删掉 `sections`（{@link #DELETABLE_SECTIONS} 的子集）。要求最近登录。含 voice 时还删本机匿名账号名下的语音样本，那一步的结果在返回值里。 */
    public VoiceSamples deleteData(List<String> sections) throws AccountStore.Failure, RecentLoginRequired {
        if (!validDeleteSections(sections)) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        RustAccount.requireUser(application, userId);
        final Object value;
        try {
            value = RustAccount.account(application, op("data_delete").put("sections", new JSONArray(sections)));
        } catch (AccountStore.Failure failure) {
            throw recentLogin(failure);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        RustAccount.requireUser(application, userId);
        return voiceSamples(value);
    }

    /** 「语音」设置里的「删除上传过的语音样本」：删掉本机匿名账号名下的样本，不要求登录（Rust 的 voice_delete）。 */
    public static void deleteVoiceSamples(Context context) throws AccountStore.Failure {
        RustAccount.account(context, op("voice_delete"));
    }

    // ---- 纯逻辑（冒烟测试覆盖） ----

    /** Rust 报的是「要求最近登录」：要求重新登录一次。 */
    public static boolean recentLoginRequired(String code) {
        return "account_recent_login_required".equals(code);
    }

    private static AccountStore.Failure recentLogin(AccountStore.Failure failure) throws RecentLoginRequired {
        if (recentLoginRequired(failure.code)) throw new RecentLoginRequired(failure);
        return failure;
    }

    /** 删除分类必须非空、属于允许集合且不能重复。 */
    public static boolean validDeleteSections(List<String> sections) {
        return sections != null && !sections.isEmpty()
            && DELETABLE_SECTIONS.containsAll(sections)
            && new HashSet<>(sections).size() == sections.size();
    }

    /** 按文件头认头像格式：PNG 签名或 JPEG 的 SOI 标记；都不是时为 null。 */
    public static String avatarType(byte[] image) {
        if (image == null || image.length < 4) return null;
        if ((image[0] & 0xFF) == 0x89 && image[1] == 'P' && image[2] == 'N' && image[3] == 'G') return "image/png";
        if ((image[0] & 0xFF) == 0xFF && (image[1] & 0xFF) == 0xD8 && (image[2] & 0xFF) == 0xFF) return "image/jpeg";
        return null;
    }

    /** 按文件头认要显示的头像格式：上传只收的 PNG、JPEG 之外还有 WebP（RIFF 容器、类型标记 WEBP）。Rust 下载头像时声明接受 WebP，Google 的头像主机按 Accept 内容协商，没上传过头像的 Google 登录用户拿到的多半是 WebP；BitmapFactory 能解。都不是时为 null。 */
    public static String displayAvatarType(byte[] image) {
        String uploadable = avatarType(image);
        if (uploadable != null) return uploadable;
        if (image != null && image.length >= 12 && image[0] == 'R' && image[1] == 'I' && image[2] == 'F' && image[3] == 'F'
            && image[8] == 'W' && image[9] == 'E' && image[10] == 'B' && image[11] == 'P') return "image/webp";
        return null;
    }

    /** Rust avatar 给的 `data:image/png;base64,…`、`data:image/jpeg;base64,…` 或 `data:image/webp;base64,…` 里的图片字节；别的形状、解不开的 base64、文件头不是这三种之一或超过 1 MiB 时为 null。 */
    public static byte[] avatarBytes(String dataUrl) {
        if (dataUrl == null) return null;
        String payload = null;
        for (String type : new String[] {"image/png", "image/jpeg", "image/webp"}) {
            String prefix = "data:" + type + ";base64,";
            if (dataUrl.startsWith(prefix)) payload = dataUrl.substring(prefix.length());
        }
        if (payload == null) return null;
        if (payload.length() > (MAX_AVATAR_BYTES + 2) / 3 * 4) return null;
        try {
            byte[] bytes = Base64.getDecoder().decode(payload);
            return displayAvatarType(bytes) == null ? null : bytes;
        } catch (IllegalArgumentException malformed) {
            return null;
        }
    }

    /** 给人看的大小：`512 B`、`3.2 KB`、`12.4 MB`、`1.1 GB`（1024 进制，一位小数）。 */
    public static String formatBytes(long bytes) {
        long value = BoundsPolicy.nonNegative(bytes);
        if (value < 1024) return value + " B";
        String[] units = {"KB", "MB", "GB", "TB"};
        double scaled = value;
        int unit = -1;
        while (scaled >= 1024 && unit < units.length - 1) {
            scaled /= 1024;
            unit++;
        }
        return NumberPolicy.decimal1(scaled) + " " + units[unit];
    }

    /** 给人看的相对时间：一分钟内「刚刚」，然后「N 分钟前」「N 小时前」「N 天前」；时间未知（0）时为空字符串。 */
    public static String relativeTime(long nowMillis, long thenMillis) {
        return RelativeTimePolicy.millisAgo(nowMillis, thenMillis);
    }

    /** 导出文件名，与服务端 `Content-Disposition` 的写法一致。 */
    public static String exportFileName(LocalDate date) {
        return "msime-data-" + date + ".zip";
    }

    /** 缓存里的每次导出使用独立路径，避免已分享的 URI 在下次导出后指向别的账号数据。 */
    public static String exportCacheFileName(LocalDate date, UUID id) {
        return "msime-data-" + date + "-" + id + ".zip";
    }

    /** RFC 3339 时间转 Unix 毫秒；读不出时为 0。 */
    static long instant(String value) {
        return RelativeTimePolicy.isoMillis(value);
    }

    // ---- 请求 ----

    private interface Request {
        Object run() throws AccountStore.Failure;
    }

    /** 请求前后都核对登录的仍是这个实例绑定的账号。 */
    private Object bound(Request request) throws AccountStore.Failure {
        RustAccount.requireUser(application, userId);
        Object value = request.run();
        RustAccount.requireUser(application, userId);
        return value;
    }

    private JSONObject call(JSONObject action) throws AccountStore.Failure {
        Object value = bound(() -> RustAccount.account(application, action));
        if (!(value instanceof JSONObject object)) throw new AccountStore.Failure("account_unavailable", 0);
        return object;
    }

    private static JSONObject op(String name) throws AccountStore.Failure {
        try {
            return new JSONObject().put("operation", name);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    /** `{voice_samples:{ok,error?,retry_after_seconds?}}`；没有这个字段（data_delete 不含 voice 时是 null）按已经删完处理。 */
    private static VoiceSamples voiceSamples(Object value) {
        JSONObject result = value instanceof JSONObject object ? object.optJSONObject("voice_samples") : null;
        if (result == null || JsonPolicy.strictTrue(result.opt("ok"))) return VoiceSamples.DONE;
        return new VoiceSamples(false, JsonPolicy.strictStringOrEmpty(result.opt("error")),
            JsonPolicy.nonNegativeLong(result.opt("retry_after_seconds"), 0));
    }

    private static Profile parseProfile(JSONObject root) throws AccountStore.Failure {
        JSONObject user = root.optJSONObject("user");
        String id = user == null ? "" : JsonPolicy.strictStringOrEmpty(user.opt("id"));
        if (id.isEmpty()) throw new AccountStore.Failure("account_unavailable", 0);
        JSONArray names = root.optJSONArray("providers");
        List<String> providers = new ArrayList<>(names == null ? 0 : names.length());
        if (names != null) {
            for (int index = 0; index < names.length(); index++) {
                String provider = JsonPolicy.strictStringOrEmpty(names.opt(index));
                if (!provider.isEmpty() && !providers.contains(provider)) providers.add(provider);
            }
        }
        return new Profile(id, JsonPolicy.strictStringOrEmpty(user.opt("displayName")),
            JsonPolicy.strictStringOrEmpty(user.opt("email")),
            !JsonPolicy.strictStringOrEmpty(user.opt("avatarUrl")).isEmpty(), ListPolicy.copyOrEmpty(providers));
    }
}
