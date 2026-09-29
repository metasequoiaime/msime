import app.msime.android.LocalAsrPolicy;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Comparator;
import java.util.stream.Stream;

/** When this host runs on-device recognition, and the hotword list it hands the native session. */
public final class LocalAsrPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws IOException {
        check(LocalAsrPolicy.usable("local", "/data/user/0/app.msime.android/files/models/sense-voice-small"), "an absolute model directory is usable");
        check(!LocalAsrPolicy.usable("openai", "/models/x"), "another provider is not local recognition");
        check(!LocalAsrPolicy.usable("local", null) && !LocalAsrPolicy.usable("local", ""), "an unset model path is not usable");
        check(!LocalAsrPolicy.usable("local", "models/x"), "a relative path is refused");
        check(!LocalAsrPolicy.usable("local", "/models/x\n/other"), "a control character is refused");
        check(!LocalAsrPolicy.usable("local", "/" + "a".repeat(LocalAsrPolicy.MAX_PATH_LENGTH)), "an overlong path is refused");

        Path root = Files.createTempDirectory("msime-local-asr");
        try {
            Path model = root.resolve("model");
            Files.createDirectories(model);
            check(!LocalAsrPolicy.installed(model.toString()), "a directory without a manifest is not installed");
            Path manifest = model.resolve(LocalAsrPolicy.MANIFEST);
            Files.write(manifest, new byte[0]);
            check(!LocalAsrPolicy.installed(model.toString()), "an empty manifest is not installed");
            Files.write(manifest, "{\"hotwords\":\"pinyin\"}".getBytes(StandardCharsets.UTF_8));
            check(LocalAsrPolicy.installed(model.toString()), "a directory with its manifest is installed");
            check(new String(LocalAsrPolicy.readManifest(model.toString()), StandardCharsets.UTF_8)
                .equals("{\"hotwords\":\"pinyin\"}"), "the manifest is read as bounded bytes");
            Path externalModel = root.resolve("external-model");
            Files.createDirectories(externalModel);
            Files.writeString(externalModel.resolve(LocalAsrPolicy.MANIFEST), "synthetic");
            Path linkedModel = root.resolve("linked-model");
            Files.createSymbolicLink(linkedModel, externalModel);
            check(!LocalAsrPolicy.installed(linkedModel.toString()), "a symlinked model directory is refused");
            boolean linkedModelRejected = false;
            try { LocalAsrPolicy.readManifest(linkedModel.toString()); }
            catch (IOException expected) { linkedModelRejected = true; }
            check(linkedModelRejected, "a symlinked model directory cannot be read");
            Files.delete(linkedModel);
            Path externalManifest = root.resolve("external-manifest.json");
            Files.writeString(externalManifest, "synthetic");
            Files.delete(manifest);
            Files.createSymbolicLink(manifest, externalManifest);
            check(!LocalAsrPolicy.installed(model.toString()), "a symlinked model manifest is refused");
            boolean linkedManifestRejected = false;
            try { LocalAsrPolicy.readManifest(model.toString()); }
            catch (IOException expected) { linkedManifestRejected = true; }
            check(linkedManifestRejected, "a symlinked model manifest cannot be read");
            Files.delete(manifest);
            Files.delete(externalManifest);
            Files.delete(externalModel.resolve(LocalAsrPolicy.MANIFEST));
            Files.delete(externalModel);
            Files.write(manifest, new byte[(int) LocalAsrPolicy.MAX_MANIFEST_BYTES + 1]);
            check(!LocalAsrPolicy.installed(model.toString()), "an oversized manifest is refused");
            boolean rejected = false;
            try { LocalAsrPolicy.readManifest(model.toString()); }
            catch (IOException expected) { rejected = true; }
            check(rejected, "an oversized manifest is rejected before allocation");
            check(!LocalAsrPolicy.installed(manifest.toString()), "a file is not a model directory");
            check(!LocalAsrPolicy.installed(root.resolve("missing").toString()) && !LocalAsrPolicy.installed(null), "a missing directory is not installed");
            Files.delete(manifest);
            Files.delete(model);
        } finally {
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }

        check(LocalAsrPolicy.correctsByPinyin("pinyin"), "pinyin mode corrects after decoding");
        check(!LocalAsrPolicy.correctsByPinyin("native") && !LocalAsrPolicy.correctsByPinyin(null), "native mode biases the decoder instead");

        check(LocalAsrPolicy.hotwordLines(null).isEmpty(), "no dictionary is no hotwords");
        check(LocalAsrPolicy.hotwordLines(Arrays.asList(" 元宇宙 ", "", null, "a\nb", "Metasequoia")).equals("元宇宙\nMetasequoia"), "blank and framing-breaking words are dropped, the rest trimmed");
        List<String> many = new ArrayList<>();
        for (int i = 0; i < LocalAsrPolicy.HOTWORD_LIMIT + 50; i++) many.add("w" + i);
        check(LocalAsrPolicy.hotwordLines(many).split("\n").length == LocalAsrPolicy.HOTWORD_LIMIT, "the list is capped at the shared limit");

        check(LocalAsrPolicy.suppliedHotword("水杉", "shui shan"), "a resolved dictionary word is carried");
        check(LocalAsrPolicy.suppliedHotword("Metasequoia", ""), "a word without pinyin still biases the decoder");
        check(!LocalAsrPolicy.suppliedHotword(null, "a") && !LocalAsrPolicy.suppliedHotword("a", null), "a missing field is dropped");
        check(!LocalAsrPolicy.suppliedHotword("  ", "kong"), "a blank word is dropped");
        check(!LocalAsrPolicy.suppliedHotword("水\n杉", "shui shan") && !LocalAsrPolicy.suppliedHotword("水杉", "shui\nshan"), "a control character is dropped");
        check(!LocalAsrPolicy.suppliedHotword("字".repeat(LocalAsrPolicy.MAX_HOTWORD_TEXT_LENGTH + 1), "zi"), "an overlong word is dropped");
        check(!LocalAsrPolicy.suppliedHotword("水杉", "a".repeat(LocalAsrPolicy.MAX_HOTWORD_PINYIN_LENGTH + 1)), "an overlong pinyin is dropped");
        System.out.println("LocalAsrPolicySmoke passed");
    }
}
