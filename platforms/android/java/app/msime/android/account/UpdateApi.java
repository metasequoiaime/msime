package app.msime.android;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.content.pm.SigningInfo;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.regex.Pattern;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 关于页「检查更新」和每天一次的自动更新共用的一层：查 msime.app 的 Android 发行版列表、按更新通道挑出比当前版本新的那一个、把本版本（edition）的 APK 下载到 `cache/updates/`，并在交给系统安装器之前核对 SHA-256 与签名证书。
 *
 * <p>发行版列表来自 `https://msime.app/api/releases?platform=android`（msime-web 带缓存的 GitHub 发行版镜像，避开 GitHub 对未鉴权请求每小时 60 次的限额），条目只有 `tag`、`version`、`prerelease` 等元数据，没有资产地址。APK 按发布流程的命名（`release-android.yml` 用 `edition_android.py` 的 `apk_name`：full 是 `msime-android.apk`，其他版本是 `msime-android-<id>.apk`）从 GitHub 发布页下载，校验值是同一发布里的 `<apk>.sha256`（`sha256sum` 的格式）。没有校验文件的发布一律不装。
 *
 * <p>每一跳请求（包括重定向）都只允许 https，且主机必须在 {@link #HOSTS} 里；重定向由这里逐跳检查而不是交给 `HttpURLConnection` 自动跟随。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；网络与文件操作都阻塞，不要在主线程调用。
 */
public final class UpdateApi {
    public static final String RELEASES_URL = "https://msime.app/api/releases?platform=android";
    /** 发布页下载地址的前缀，后面接 `<tag>/<资产名>`。 */
    public static final String DOWNLOAD_PREFIX = "https://github.com/metasequoiaime/msime/releases/download/";
    /** 允许连接的主机：msime.app 的发行版列表，GitHub 发布页和它重定向到的两个资产域名。 */
    public static final Set<String> HOSTS = Set.of(
        "msime.app", "github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com");
    /** 下载到应用缓存下的这个子目录，FileProvider 的 `updates` 路径指向它。 */
    public static final String CACHE_DIRECTORY = "updates";
    static final int MAX_LIST_BYTES = 2 * 1024 * 1024;
    static final int MAX_CHECKSUM_BYTES = 4 * 1024;
    /** APK 大小上限；超过就不是我们发的包。 */
    static final long MAX_APK_BYTES = 512L * 1024 * 1024;
    static final int MAX_REDIRECTS = 5;
    static final int CONNECT_TIMEOUT_MILLIS = 15_000;
    static final int READ_TIMEOUT_MILLIS = 60_000;
    private static final Pattern EDITION_ID = Pattern.compile("[a-z][a-z0-9]{0,31}");
    private static final Pattern TAG = Pattern.compile("[A-Za-z0-9][A-Za-z0-9._+-]{0,127}");
    private static final Pattern SHA256 = Pattern.compile("[0-9a-f]{64}");
    /** 同一时刻只让一次下载动 `updates/` 目录：关于页的手动更新和 UpdateJobService 的每日任务都在主进程里，同时下载时会互删对方的 `.part`、写同一个文件。 */
    private static final Object DOWNLOAD_LOCK = new Object();

    /** 更新通道：稳定版只看正式发布，预览版正式与预发布都看。 */
    public enum Channel {
        STABLE("stable", "稳定版"),
        PREVIEW("preview", "预览版");

        private final String id;
        private final String title;

        Channel(String id, String title) {
            this.id = id;
            this.title = title;
        }

        public String id() { return id; }

        public String title() { return title; }

        /** 存储里读出的值不认识时按稳定版处理。 */
        public static Channel fromId(String id) {
            return PREVIEW.id.equals(id) ? PREVIEW : STABLE;
        }

        public boolean accepts(Release release) {
            return this == PREVIEW || !release.prerelease();
        }
    }

    /** 发行版列表里的一项。 */
    public record Release(String tag, String version, boolean prerelease) {}

    /** 一个可以下载的更新：哪个发布、APK 与校验文件的地址、下载后的文件名。 */
    public record Update(Release release, String apkUrl, String checksumUrl, String fileName) {}

    /** 检查或安装失败的原因；`message` 是给用户看的中文。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;

        public Failure(String message) {
            super(message);
        }

        public Failure(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /** 真正发 GET 请求的那一层，冒烟测试换成内存实现；`url` 已经过白名单检查。 */
    public interface Transport {
        /** 返回状态码、`Location` 与响应体流；调用方负责关闭流。 */
        Exchange get(String url) throws IOException;
    }

    /** 一次 GET 的结果。 */
    public record Exchange(int status, String location, long length, InputStream body) {}

    private final Transport transport;

    public UpdateApi() {
        this(UpdateApi::httpGet);
    }

    public UpdateApi(Transport transport) {
        this.transport = transport;
    }

    /** 本版本发布时的 APK 资产名（不含 `.apk`），与 `edition_android.py` 的 `apk_name` 一致。 */
    public static String apkAssetName(String edition) {
        if (edition == null || !EDITION_ID.matcher(edition).matches()) {
            throw new IllegalArgumentException("not an edition id: " + edition);
        }
        return "full".equals(edition) ? "msime-android" : "msime-android-" + edition;
    }

    /** 地址是 https、主机在白名单里、没有用户信息和非默认端口时为真。 */
    public static boolean allowedUrl(String url) {
        if (url == null) return false;
        try {
            URI uri = new URI(url);
            if (!"https".equalsIgnoreCase(uri.getScheme())) return false;
            if (uri.getRawUserInfo() != null) return false;
            if (uri.getPort() != -1 && uri.getPort() != 443) return false;
            String host = uri.getHost();
            return host != null && HOSTS.contains(TextPolicy.lowercase(host));
        } catch (URISyntaxException malformed) {
            return false;
        }
    }

    /** 重定向的 `Location` 相对 `base` 解析成绝对地址；解析不了时返回 null。 */
    static String resolve(String base, String location) {
        if (location == null || location.isEmpty()) return null;
        try {
            return new URI(base).resolve(location).toString();
        } catch (URISyntaxException | IllegalArgumentException malformed) {
            return null;
        }
    }

    /**
     * 比较两个版本号：`.` 分隔的数字段逐段比，缺的段当 0；主版本相同时，没有 `-` 后缀的正式版比带后缀的预发布版新，两个后缀按 `.` 分段，数字段按数值、其他按字典序。
     *
     * @return 负数表示 `left` 更旧，0 表示相同，正数表示 `left` 更新
     */
    public static int compareVersions(String left, String right) {
        String[] a = splitVersion(left);
        String[] b = splitVersion(right);
        int core = compareSegments(a[0].split("\\."), b[0].split("\\."), true);
        if (core != 0) return core;
        if (a[1].isEmpty() && b[1].isEmpty()) return 0;
        if (a[1].isEmpty()) return 1;
        if (b[1].isEmpty()) return -1;
        return compareSegments(a[1].split("\\."), b[1].split("\\."), false);
    }

    private static String[] splitVersion(String version) {
        String value = TextPolicy.trimmed(version);
        if (value.startsWith("v") || value.startsWith("V")) value = value.substring(1);
        int plus = value.indexOf('+');
        if (plus >= 0) value = value.substring(0, plus);
        int dash = value.indexOf('-');
        return dash < 0 ? new String[] {value, ""} : new String[] {value.substring(0, dash), value.substring(dash + 1)};
    }

    private static int compareSegments(String[] a, String[] b, boolean padWithZero) {
        int length = BoundsPolicy.atLeast(a.length, b.length);
        for (int index = 0; index < length; index++) {
            if (!padWithZero && (index >= a.length || index >= b.length)) return Integer.compare(a.length, b.length);
            String x = index < a.length ? a[index] : "0";
            String y = index < b.length ? b[index] : "0";
            boolean numericX = numeric(x);
            boolean numericY = numeric(y);
            int order;
            if (numericX && numericY) order = compareNumeric(x, y);
            else if (numericX) order = -1;
            else if (numericY) order = 1;
            else order = x.compareTo(y);
            if (order != 0) return order;
        }
        return 0;
    }

    private static boolean numeric(String segment) {
        if (segment.isEmpty()) return false;
        for (int index = 0; index < segment.length(); index++) {
            char c = segment.charAt(index);
            if (c < '0' || c > '9') return false;
        }
        return true;
    }

    /** Compare non-empty decimal strings without converting them to a fixed-width integer. */
    private static int compareNumeric(String left, String right) {
        int leftStart = 0;
        while (leftStart + 1 < left.length() && left.charAt(leftStart) == '0') leftStart++;
        int rightStart = 0;
        while (rightStart + 1 < right.length() && right.charAt(rightStart) == '0') rightStart++;
        int length = Integer.compare(left.length() - leftStart, right.length() - rightStart);
        if (length != 0) return length;
        return left.substring(leftStart).compareTo(right.substring(rightStart));
    }

    /** 从列表里挑出通道接受、比 `currentVersion` 新的最高版本；没有时返回 null。 */
    public static Release pick(List<Release> releases, Channel channel, String currentVersion) {
        Release best = null;
        for (Release release : releases) {
            if (!channel.accepts(release)) continue;
            if (compareVersions(release.version(), currentVersion) <= 0) continue;
            if (best == null || compareVersions(release.version(), best.version()) > 0) best = release;
        }
        return best;
    }

    /** 由发布和本版本拼出下载地址；tag 不像一个发布标签时返回 null。 */
    public static Update update(Release release, String edition) {
        if (release == null || release.tag() == null || !TAG.matcher(release.tag()).matches()) return null;
        String file = apkAssetName(edition) + ".apk";
        String apk = DOWNLOAD_PREFIX + release.tag() + "/" + file;
        return new Update(release, apk, apk + ".sha256", file);
    }

    /** 读 `sha256sum` 的输出：第一个字段是 64 位十六进制；读不出时返回 null。 */
    public static String parseChecksum(String sidecar) {
        if (sidecar == null) return null;
        String trimmed = TextPolicy.trimmed(sidecar);
        int end = 0;
        while (end < trimmed.length() && !Character.isWhitespace(trimmed.charAt(end))) end++;
        String digest = TextPolicy.lowercase(trimmed.substring(0, end));
        return SHA256.matcher(digest).matches() ? digest : null;
    }

    /** 读 msime.app 的发行版列表：`{items:[{tag,version,prerelease,…}]}`，只保留平台是 android 的条目。 */
    public static List<Release> parseReleases(String json) throws Failure {
        try {
            JSONObject root = new JSONObject(json);
            JSONArray items = root.optJSONArray("items");
            if (items == null) throw new Failure("发行版列表格式不对");
            List<Release> releases = new ArrayList<>(items.length());
            for (int index = 0; index < items.length(); index++) {
                JSONObject item = items.optJSONObject(index);
                if (item == null) continue;
                Object rawPlatform = item.opt("platform");
                String platform = rawPlatform == null || rawPlatform == JSONObject.NULL
                    ? "android" : JsonPolicy.strictString(rawPlatform);
                if (!"android".equals(platform)) continue;
                Object prerelease = item.opt("prerelease");
                String tag = JsonPolicy.strictString(item.opt("tag"));
                String version = JsonPolicy.strictString(item.opt("version"));
                if (tag == null || version == null || tag.isEmpty() || version.isEmpty()
                        || !(prerelease instanceof Boolean)) continue;
                releases.add(new Release(tag, version, (Boolean) prerelease));
            }
            return releases;
        } catch (JSONException malformed) {
            throw new Failure("发行版列表格式不对", malformed);
        }
    }

    /** 查更新：有比当前版本新的发布时返回它，否则返回 null。 */
    public Update check(Channel channel, String edition, String currentVersion) throws Failure {
        byte[] body = fetch(RELEASES_URL, MAX_LIST_BYTES);
        Release release = pick(parseReleases(new String(body, StandardCharsets.UTF_8)), channel, currentVersion);
        return update(release, edition);
    }

    /** 下载进度：已下载字节与总字节（不知道总长时为 -1）。 */
    public interface Progress {
        void onProgress(long done, long total);
    }

    /**
     * 把更新下载到 `cacheDir/updates/` 并核对 SHA-256，返回核对过的文件。目录里别的旧安装包一并删掉。
     *
     * <p>先下到 `.part` 临时文件，核对通过才改名，核对失败的文件不会留下。两次下载同时发生时排队（{@link #DOWNLOAD_LOCK}）；后一次拿到锁时如果前一次已经把同一个版本下好并核对过，直接用它，不再下载一遍。
     */
    public File download(Update update, File cacheDir, Progress progress) throws Failure {
        String expected = parseChecksum(new String(fetch(update.checksumUrl(), MAX_CHECKSUM_BYTES), StandardCharsets.UTF_8));
        if (expected == null) throw new Failure("这个版本没有校验信息，请到官网下载");
        synchronized (DOWNLOAD_LOCK) {
            return downloadLocked(update, cacheDir, progress, expected);
        }
    }

    private File downloadLocked(Update update, File cacheDir, Progress progress, String expected) throws Failure {
        if (cacheDir == null) throw new Failure("没有空间存放安装包");
        java.nio.file.Path cachePath = cacheDir.toPath();
        if (Files.isSymbolicLink(cachePath)) throw new Failure("更新目录不安全");
        File directory = new File(cacheDir, CACHE_DIRECTORY);
        java.nio.file.Path directoryPath = directory.toPath();
        if (Files.isSymbolicLink(directoryPath)) throw new Failure("更新目录不安全");
        if (!Files.isDirectory(directoryPath, LinkOption.NOFOLLOW_LINKS)
                && !directory.mkdirs()) throw new Failure("没有空间存放安装包");
        if (!Files.isDirectory(directoryPath, LinkOption.NOFOLLOW_LINKS))
            throw new Failure("更新目录不安全");
        File[] stale = directory.listFiles();
        if (stale != null) {
            for (File file : stale) {
                if (!file.getName().equals(update.fileName())) deleteQuietly(file);
            }
        }
        File target = new File(directory, update.fileName());
        if (verified(target, expected)) return target;
        File partial = new File(directory, update.fileName() + ".part");
        MessageDigest digest = sha256();
        Exchange response = open(update.apkUrl());
        try (InputStream body = response.body()) {
            long total = response.length();
            try (OutputStream out = Files.newOutputStream(partial.toPath(),
                    StandardOpenOption.CREATE, StandardOpenOption.TRUNCATE_EXISTING,
                    StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS)) {
                byte[] buffer = new byte[64 * 1024];
                long done = 0;
                for (int read; (read = body.read(buffer)) != -1; ) {
                    done += read;
                    if (done > MAX_APK_BYTES) throw new Failure("安装包大小不对");
                    digest.update(buffer, 0, read);
                    out.write(buffer, 0, read);
                    if (progress != null) progress.onProgress(done, total);
                }
            }
        } catch (IOException offline) {
            deleteQuietly(partial);
            throw new Failure("下载没有完成，请检查网络后重试", offline);
        } catch (Failure failure) {
            deleteQuietly(partial);
            throw failure;
        }
        if (!MessageDigest.isEqual(hex(digest.digest()).getBytes(StandardCharsets.US_ASCII),
                expected.getBytes(StandardCharsets.US_ASCII))) {
            deleteQuietly(partial);
            throw new Failure("安装包校验不通过，已删除");
        }
        deleteQuietly(target);
        if (!partial.renameTo(target)) {
            deleteQuietly(partial);
            throw new Failure("没有空间存放安装包");
        }
        return target;
    }

    /**
     * 安装前的最后一道检查：包名与本应用相同、版本号更高、签名证书与当前安装的包一致（签名轮换过的包要在证书历史里包含当前证书）。任何一项读不出都按不通过处理。
     */
    public static void verifyArchive(Context context, File apk) throws Failure {
        PackageManager manager = context.getPackageManager();
        PackageInfo archive = manager.getPackageArchiveInfo(apk.getAbsolutePath(), PackageManager.GET_SIGNING_CERTIFICATES);
        PackageInfo installed;
        try {
            installed = manager.getPackageInfo(context.getPackageName(), PackageManager.GET_SIGNING_CERTIFICATES);
        } catch (PackageManager.NameNotFoundException missing) {
            throw new Failure("读不到当前安装的版本", missing);
        }
        if (archive == null) throw new Failure("安装包无法解析，已删除");
        if (!context.getPackageName().equals(archive.packageName)) throw new Failure("安装包不是这个应用的");
        if (archive.getLongVersionCode() <= installed.getLongVersionCode()) throw new Failure("安装包不比当前版本新");
        if (!sameSigner(archive.signingInfo, installed.signingInfo)) throw new Failure("安装包的签名与当前版本不一致，已删除");
    }

    /** 签名比较：多签名的包要求签名集合完全相同，单签名的包要求新包的证书历史里有当前证书。 */
    static boolean sameSigner(SigningInfo archive, SigningInfo installed) {
        if (archive == null || installed == null) return false;
        Set<String> current = fingerprints(installed.hasMultipleSigners()
            ? installed.getApkContentsSigners() : installed.getSigningCertificateHistory());
        if (current.isEmpty()) return false;
        if (archive.hasMultipleSigners() || installed.hasMultipleSigners()) {
            if (!archive.hasMultipleSigners() || !installed.hasMultipleSigners()) return false;
            return fingerprints(archive.getApkContentsSigners()).equals(current);
        }
        Set<String> history = fingerprints(archive.getSigningCertificateHistory());
        Signature[] now = installed.getApkContentsSigners();
        if (now == null || now.length == 0) return false;
        for (Signature signature : now) {
            if (!history.contains(hex(sha256().digest(signature.toByteArray())))) return false;
        }
        return true;
    }

    private static Set<String> fingerprints(Signature[] signatures) {
        Set<String> result = new HashSet<>(signatures == null ? 0 : signatures.length);
        if (signatures == null) return result;
        for (Signature signature : signatures) result.add(hex(sha256().digest(signature.toByteArray())));
        return result;
    }

    /** 已经下好的安装包是否就是这一版：读不了按没有处理，照常重新下载。 */
    private static boolean verified(File file, String expected) {
        if (!file.isFile()) return false;
        try {
            return MessageDigest.isEqual(sha256Hex(file).getBytes(StandardCharsets.US_ASCII),
                expected.getBytes(StandardCharsets.US_ASCII));
        } catch (IOException unreadable) {
            return false;
        }
    }

    /** 文件的 SHA-256 十六进制。 */
    public static String sha256Hex(File file) throws IOException {
        return DigestPolicy.sha256Hex(file);
    }

    static String hex(byte[] bytes) {
        return DigestPolicy.hex(bytes);
    }

    private static MessageDigest sha256() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static void deleteQuietly(File file) {
        if (file.exists() && !file.delete()) file.deleteOnExit();
    }

    /** 逐跳跟随重定向，每一跳都过白名单；返回最终的 200 响应，调用方关闭它的流。 */
    private Exchange open(String url) throws Failure {
        String current = url;
        for (int hop = 0; hop <= MAX_REDIRECTS; hop++) {
            if (!allowedUrl(current)) throw new Failure("更新地址不在允许的范围内");
            Exchange exchange;
            try {
                exchange = transport.get(current);
            } catch (IOException offline) {
                throw new Failure("连不上更新服务器，请检查网络后重试", offline);
            }
            int status = exchange.status();
            if (status == 200 && exchange.body() != null) {
                if (exchange.length() > MAX_APK_BYTES) {
                    closeQuietly(exchange.body());
                    throw new Failure("安装包大小不对");
                }
                return exchange;
            }
            closeQuietly(exchange.body());
            if (status == 301 || status == 302 || status == 303 || status == 307 || status == 308) {
                current = resolve(current, exchange.location());
                if (current == null) throw new Failure("更新地址不在允许的范围内");
                continue;
            }
            if (status == 404) throw new Failure("这个版本没有本设备可用的安装包");
            throw new Failure("更新服务器暂时不可用（HTTP " + status + "）");
        }
        throw new Failure("更新地址跳转次数过多");
    }

    /** GET 一个小文件并整个读进内存。 */
    byte[] fetch(String url, int maxBytes) throws Failure {
        try (InputStream body = open(url).body()) {
            byte[] response = HttpBodyPolicy.readBounded(body, maxBytes);
            if (response == null) throw new Failure("更新服务器的响应过大");
            return response;
        } catch (IOException offline) {
            throw new Failure("连不上更新服务器，请检查网络后重试", offline);
        }
    }

    private static void closeQuietly(InputStream stream) {
        if (stream == null) return;
        try {
            stream.close();
        } catch (IOException ignored) {
            // 丢弃的响应体关不上不影响下一跳。
        }
    }

    private static Exchange httpGet(String url) throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(url).openConnection();
        connection.setInstanceFollowRedirects(false);
        connection.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
        connection.setReadTimeout(READ_TIMEOUT_MILLIS);
        connection.setRequestProperty("User-Agent", CloudApi.USER_AGENT);
        connection.setRequestProperty("Accept", "*/*");
        int status = connection.getResponseCode();
        InputStream body = status >= HttpURLConnection.HTTP_BAD_REQUEST ? connection.getErrorStream() : connection.getInputStream();
        return new Exchange(status, connection.getHeaderField("Location"), connection.getContentLengthLong(), body);
    }

    /** Compatibility entry point retained for the host smoke contract. */
    static String strictString(Object value) {
        return JsonPolicy.strictString(value);
    }
}
