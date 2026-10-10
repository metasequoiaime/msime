package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * Whether a resolved configuration is on-device recognition this host can run, and the small rules around it.
 *
 * <p>Nothing about the provider is resolved here: the shared layer (`msime_client_mobile_voice_configuration`) decides that provider `local` with a model path is what the user configured. This class only checks what this host is about to hand to the recognizer, and holds the pieces that are worth testing without a device: the path check, the installed-model check and the hotword wire format.
 */
public final class LocalAsrPolicy {
    public static final String PROVIDER = "local";
    /** Written last by the shared installer, so a half-installed directory never looks usable. */
    public static final String MANIFEST = "msime-model.json";
    /** The same ceiling the shared preferences put on `asr_model_path`. */
    public static final int MAX_PATH_LENGTH = 4096;
    /** The shared default; the recognizer itself keeps at most this many. */
    public static final int HOTWORD_LIMIT = 200;
    /** A loaded model holds hundreds of megabytes; one not used for this long is dropped and reloaded by the next dictation. Matches the desktop helper. */
    public static final long IDLE_RELEASE_MILLIS = 120_000;
    /** The same cap the other recording paths use, so a forgotten session cannot hold the microphone. */
    public static final int MAX_MILLIS = 60_000;
    /** The manifest is the catalog entry, a few kilobytes; anything far larger is not one. */
    public static final long MAX_MANIFEST_BYTES = 256 * 1024;
    /** Per-word ceilings of a hotword handed in by the shared layer, in UTF-8 bytes. */
    public static final int MAX_HOTWORD_TEXT_LENGTH = 256;
    public static final int MAX_HOTWORD_PINYIN_LENGTH = 1024;
    /** 本机模型输出与网络识别结果使用同一长度上限。 */
    public static final int MAX_TRANSCRIPT = 2000;

    private LocalAsrPolicy() {}

    /**
     * Whether this is on-device recognition with a model path this host may open.
     *
     * <p>Only a directory the shared installer produced can be used here; a Whisper model file, which desktop hosts also accept as `asr_model_path`, has no recognizer on Android. That is reported when the dictation starts ({@link #installed}), not by falling back to the platform recogniser: a user who chose on-device recognition did not agree to the audio going to whatever service the device ships.
     */
    public static boolean usable(String provider, String modelPath) {
        if (!PROVIDER.equals(provider) || modelPath == null) return false;
        if (modelPath.isEmpty() || TextPolicy.utf8Length(modelPath) > MAX_PATH_LENGTH) return false;
        if (TextPolicy.hasControl(modelPath) || !TextPolicy.validUnicode(modelPath)) return false;
        return modelPath.startsWith("/");
    }

    /** Whether `modelPath` is an installed model directory below the trusted app files root. */
    public static boolean installed(String modelPath, Path trustedRoot) {
        if (modelPath == null || modelPath.isEmpty() || trustedRoot == null) return false;
        try {
            Path model = trustedModelPath(modelPath, trustedRoot);
            Path manifest = model.resolve(MANIFEST);
            return Files.isDirectory(model, LinkOption.NOFOLLOW_LINKS)
                && Files.isRegularFile(manifest, LinkOption.NOFOLLOW_LINKS)
                && SafePaths.isSingleLink(manifest)
                && Files.size(manifest) > 0 && Files.size(manifest) <= MAX_MANIFEST_BYTES;
        } catch (java.nio.file.InvalidPathException | IOException | SecurityException error) {
            return false;
        }
    }

    /** Reads the manifest with a hard cap, so a file that grows after inspection cannot cause an unbounded allocation. */
    public static byte[] readManifest(String modelPath, Path trustedRoot) throws IOException {
        if (modelPath == null || modelPath.isEmpty() || trustedRoot == null)
            throw new IOException("manifest unavailable");
        final Path model;
        try {
            model = trustedModelPath(modelPath, trustedRoot);
        } catch (java.nio.file.InvalidPathException error) {
            throw new IOException("manifest unavailable", error);
        }
        if (!Files.isDirectory(model, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("manifest unavailable");
        Path manifest = model.resolve(MANIFEST);
        if (!Files.isRegularFile(manifest, LinkOption.NOFOLLOW_LINKS)
                || !SafePaths.isSingleLink(manifest))
            throw new IOException("manifest unavailable");
        try (InputStream input = Files.newInputStream(manifest, LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readRequired(input, (int) MAX_MANIFEST_BYTES);
            if (bytes.length == 0) throw new IOException("manifest empty");
            return bytes;
        }
    }

    /** Resolve a model path only when every component below the app's trusted files root is a real directory. */
    private static Path trustedModelPath(String modelPath, Path trustedRoot) throws IOException {
        if (!trustedRoot.isAbsolute()) throw new IOException("manifest unavailable");
        Path root = trustedRoot.normalize();
        Path model = Paths.get(modelPath);
        if (!model.isAbsolute()) throw new IOException("manifest unavailable");
        model = model.normalize();
        if (!model.startsWith(root) || model.equals(root)) throw new IOException("manifest unavailable");
        if (Files.isSymbolicLink(root)
                || !Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("manifest unavailable");
        Path current = root;
        for (Path component : root.relativize(model)) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current)
                    || (Files.exists(current, LinkOption.NOFOLLOW_LINKS)
                        && !Files.isDirectory(current, LinkOption.NOFOLLOW_LINKS))) {
                throw new IOException("manifest unavailable");
            }
        }
        return model;
    }

    /** Whether the model's manifest asks for post-correction rather than native biasing. */
    public static boolean correctsByPinyin(String hotwordMode) {
        return "pinyin".equals(hotwordMode);
    }

    /**
     * 本机识别被拒时给用户看的统一说明。
     *
     * @return 用户主动取消时为 null，其余失败返回可直接展示的文案
     */
    public static String failureMessage(LocalAsrRecognizer.Failure failure, boolean runtimeMissing) {
        return switch (failure) {
            case PERMISSION -> "语音识别需要麦克风权限";
            case UNAVAILABLE -> "麦克风被其他应用占用";
            case MODEL -> "本地语音模型未安装或已损坏，请在设置中重新下载";
            case RUNTIME -> runtimeMissing
                ? "本地语音识别组件尚未下载，请在设置中下载后再试"
                : "本地语音识别组件无法加载";
            case EMPTY -> "没有听到内容";
            case CANCELLED -> null;
        };
    }

    /** Native ASR text must be plain, well-formed Unicode before it reaches the editor. */
    static String transcript(Object value) {
        String text = JsonPolicy.strictString(value);
        if (text == null || TextPolicy.codePointLength(text) > MAX_TRANSCRIPT
                || TextPolicy.hasControlExceptWhitespace(text)
                || !TextPolicy.validUnicode(text)) return "";
        return text;
    }

    /**
     * Whether one `{text, pinyin}` hotword the shared layer resolved (the Tauri request's `hotwords`) may be carried to the recognizer.
     *
     * <p>The shared layer already built the list from the user's dictionary; this only keeps what crosses the activity boundary bounded and free of anything that could break the one-per-line framing. An empty pinyin is allowed: the native bias needs only the text, and the pinyin correction skips a word it cannot match.
     */
    public static boolean suppliedHotword(String text, String pinyin) {
        if (text == null || pinyin == null) return false;
        String trimmed = TextPolicy.trimmed(text);
        if (trimmed.isEmpty() || TextPolicy.utf8Length(text) > MAX_HOTWORD_TEXT_LENGTH) return false;
        if (TextPolicy.utf8Length(pinyin) > MAX_HOTWORD_PINYIN_LENGTH) return false;
        return !TextPolicy.hasControl(text) && !TextPolicy.hasControl(pinyin)
            && TextPolicy.validUnicode(text) && TextPolicy.validUnicode(pinyin);
    }

    /**
     * Hotword texts in the form the native session takes them: one per line, at most {@link #HOTWORD_LIMIT}.
     *
     * <p>A word that could break the framing (a line break or any other control character) or is blank is dropped rather than repaired; the list is a bias, and a missing entry costs nothing.
     */
    public static String hotwordLines(List<String> words) {
        if (words == null) return "";
        int capacity = BoundsPolicy.bounded(words.size(), 0, HOTWORD_LIMIT)
            * (MAX_HOTWORD_TEXT_LENGTH + 1);
        StringBuilder out = new StringBuilder(capacity);
        int kept = 0;
        for (String word : words) {
            if (kept == HOTWORD_LIMIT) break;
            if (word == null) continue;
            String trimmed = TextPolicy.trimmed(word);
            if (trimmed.isEmpty() || TextPolicy.hasControl(trimmed)
                    || !TextPolicy.validUnicode(trimmed)) continue;
            if (kept > 0) out.append('\n');
            out.append(trimmed);
            kept++;
        }
        return out.toString();
    }
}
