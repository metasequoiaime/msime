import app.msime.android.NativeClient;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.regex.Pattern;

public final class NativeSmoke {
    static void success(String response) { if (!response.contains("\"ok\":true")) throw new AssertionError(response); }
    public static void main(String[] args) throws Exception {
        if (!NativeClient.prepareHost("{\"resources\":\"relative\",\"state_root\":\"relative\"}").contains("\"ok\":false")) {
            throw new AssertionError("bootstrap accepted relative paths");
        }
        if (!NativeClient.snapshotVersion("{\"resources\":\"relative\"}").contains("\"ok\":false")) {
            throw new AssertionError("snapshot version accepted incomplete paths");
        }
        if (!NativeClient.snapshotPrepare("{}", "relative").contains("\"ok\":false")) {
            throw new AssertionError("snapshot preparation accepted an invalid request");
        }
        if (!NativeClient.emojiCatalog("{\"cursor\":true,\"limit\":64}", "relative")
                .contains("\"ok\":false")) {
            throw new AssertionError("emoji catalog accepted a relative resource path");
        }
        if (!NativeClient.candidateGlosses(
                "{\"generation\":1,\"candidates\":[{\"text\":\"fixture\",\"source\":0}]}",
                "relative").contains("\"ok\":false")) {
            throw new AssertionError("candidate gloss accepted a relative resource path");
        }
        translationServices();
        Path root = Files.createTempDirectory("msime-jni-");
        try {
            Path preferences = Files.createDirectory(root.resolve("preferences-🌲"));
            success(NativeClient.loadPreferences(preferences.toString()));
            Path saved = preferences.resolve("preferences.json");
            String firstSnapshot = "{\"format_version\":1,\"revision\":0,\"preferences\":{"
                + "\"scheme\":\"quanpin\",\"candidate_page_size\":5,\"learning\":false,"
                + "\"chinese_punctuation\":true}}";
            String firstSave = NativeClient.savePreferences(preferences.toString(), 0, firstSnapshot);
            success(firstSave);
            if (!firstSave.contains("\"revision\":1")) throw new AssertionError(firstSave);
            if (!NativeClient.savePreferences(preferences.toString(), 0, firstSnapshot).contains("\"ok\":false")) {
                throw new AssertionError("stale preferences save was accepted");
            }
            Files.writeString(saved, "broken");
            if (!NativeClient.loadPreferences(preferences.toString()).contains("\"ok\":false") || !Files.readString(saved).equals("broken")) {
                throw new AssertionError("malformed preferences were accepted or overwritten");
            }
            if (!NativeClient.loadPreferences("relative").contains("\"ok\":false")) throw new AssertionError("relative preferences directory accepted");
            StringBuilder options = new StringBuilder("{\"api_version\":1,");
            for (String name : new String[] { "resources", "user_data", "cache", "dictionaries" }) {
                Path directory = Files.createDirectory(root.resolve(name + "-🌲"));
                String path = directory.toString().replace("\\", "\\\\").replace("\"", "\\\"");
                options.append('"').append(name).append("\":\"").append(path).append("\",");
            }
            options.append("\"preferences\":{\"scheme\":\"quanpin\",\"candidate_page_size\":5,\"learning\":false,\"chinese_punctuation\":true}}");
            String created = NativeClient.create(options.toString());
            success(created);
            var matcher = Pattern.compile("\"session\":(\\d+)").matcher(created);
            if (!matcher.find()) throw new AssertionError(created);
            long handle = Long.parseLong(matcher.group(1));
            var generationMatcher = Pattern.compile("\"generation\":(\\d+)").matcher(created);
            if (!generationMatcher.find()) throw new AssertionError(created);
            success(NativeClient.applyTranslations(handle,
                Long.parseLong(generationMatcher.group(1)), "[]"));
            success(NativeClient.focus(handle, true));
            success(NativeClient.setEnglishMode(handle, true));
            String english = NativeClient.character(handle, 'H', true);
            success(english);
            if (!english.contains("\"editing_text\":\"H\"")) throw new AssertionError(english);
            String englishCommit = NativeClient.command(handle, 1);
            success(englishCommit);
            if (!englishCommit.contains("\"commit\":\"H\"")) throw new AssertionError(englishCommit);
            success(NativeClient.setEnglishMode(handle, false));
            String smartAscii = NativeClient.punctuationWithContext(handle, ',', 'A');
            success(smartAscii);
            if (!smartAscii.contains("\"handled\":false")) throw new AssertionError(smartAscii);
            String smartChinese = NativeClient.punctuationWithContext(handle, ',', '中');
            success(smartChinese);
            if (!smartChinese.contains("\"commit\":\"，\"")) throw new AssertionError(smartChinese);
            success(NativeClient.character(handle, 'n', false));
            success(NativeClient.character(handle, 'i', false));
            String composedPunctuation = NativeClient.punctuationWithContext(handle, ',', 'A');
            success(composedPunctuation);
            if (!composedPunctuation.contains("，\"")) throw new AssertionError(composedPunctuation);
            String nineKey = NativeClient.setNineKeyMode(handle, true);
            success(nineKey);
            if (!nineKey.contains("\"nine_key\":true")) throw new AssertionError(nineKey);
            String digit = NativeClient.character(handle, '6', false);
            success(digit);
            if (!digit.contains("\"handled\":true") || !digit.contains("\"nine_key_spellings\":[")) {
                throw new AssertionError(digit);
            }
            var nineGeneration = Pattern.compile("\"generation\":(\\d+)").matcher(digit);
            if (!nineGeneration.find()) throw new AssertionError(digit);
            String single = NativeClient.setNineKeyFilter(handle, true, "");
            success(single);
            if (!single.contains("\"nine_key_single_character\":true")) throw new AssertionError(single);
            String unfiltered = NativeClient.setNineKeyFilter(handle, false, "");
            success(unfiltered);
            nineGeneration = Pattern.compile("\"generation\":(\\d+)").matcher(unfiltered);
            if (!nineGeneration.find()) throw new AssertionError(unfiltered);
            success(NativeClient.chooseNineKeySpelling(handle,
                Long.parseLong(nineGeneration.group(1)), 0));
            success(NativeClient.command(handle, 3));
            success(NativeClient.setNineKeyMode(handle, false));
            success(NativeClient.character(handle, 'U', true));
            for (char value : "1f332".toCharArray()) success(NativeClient.character(handle, value, false));
            String snapshot = "{\"format_version\":1,\"revision\":1,\"preferences\":{\"scheme\":\"quanpin\",\"candidate_page_size\":2,\"learning\":false,\"chinese_punctuation\":false}}";
            Files.writeString(saved, snapshot);
            var loaded = new java.util.concurrent.atomic.AtomicReference<String>();
            Thread reader = new Thread(() -> loaded.set(NativeClient.loadPreferences(preferences.toString())));
            reader.start();
            reader.join();
            success(loaded.get());
            if (!loaded.get().contains("\"revision\":1")) throw new AssertionError("background reader lost revision");
            String queued = NativeClient.updatePreferences(handle, snapshot);
            success(queued);
            if (!queued.contains("\"deferred\":true")) throw new AssertionError(queued);
            String committed = NativeClient.command(handle, 1);
            success(committed);
            if (!committed.contains("\"commit\":\"🌲\"")) throw new AssertionError(committed);
            String updated = NativeClient.updatePreferences(handle, snapshot);
            success(updated);
            if (!updated.contains("\"deferred\":false")) throw new AssertionError(updated);
            String punctuation = NativeClient.character(handle, ',', false);
            success(punctuation);
            if (!punctuation.contains("\"handled\":false")) throw new AssertionError(punctuation);
            success(NativeClient.destroy(handle));
            if (!NativeClient.view(handle).contains("\"ok\":false")) throw new AssertionError("stale handle accepted");
            System.out.println("JNI consumer: English mode, smart punctuation, nine-key, UTF-8 paths, preferences CAS and input commit passed");
        } finally {
            try (var paths = Files.walk(root)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    /** 候选翻译接用户自己的服务时经过的 JNI 导出：描述器、回复解析、失败判断和设置页检查，全部用合成配置，不联网。 */
    private static void translationServices() {
        String deeplx = NativeClient.customTranslationHttpRequest("{\"config\":{\"enabled\":true,"
            + "\"endpoint\":\"http://127.0.0.1:1188/translate\",\"api_key\":\"\"},\"text\":\"你好\","
            + "\"source_language\":\"zh\",\"target_language\":\"en\"}");
        success(deeplx);
        if (!deeplx.contains("\"url\":\"http://127.0.0.1:1188/translate\"") || !deeplx.contains("\"headers\":"))
            throw new AssertionError("DeepLX descriptor lacks its url or headers");
        if (!NativeClient.customTranslationHttpRequest("{\"config\":{\"enabled\":true,"
                + "\"endpoint\":\"https://api-free.deepl.com\",\"api_key\":\"\"},\"text\":\"你好\","
                + "\"source_language\":\"zh\",\"target_language\":\"en\"}").contains("\"ok\":false"))
            throw new AssertionError("DeepL descriptor was built without a key");
        String tencent = NativeClient.tencentTranslationHttpRequest("{\"config\":{\"enabled\":true,"
            + "\"secret_id\":\"AKIDsynthetic\",\"secret_key\":\"synthetic\",\"region\":\"\"},"
            + "\"texts\":[\"你好\"],\"source_language\":\"zh\",\"target_language\":\"en\",\"timestamp\":1704067200}");
        success(tencent);
        if (!tencent.contains("\"body_utf8\":")) throw new AssertionError("Tencent descriptor lacks its signed body");
        String niutrans = NativeClient.niutransTranslationHttpRequest("{\"config\":{\"enabled\":true,"
            + "\"app_id\":\"app-id\",\"apikey\":\"api-key\"},\"text\":\"你好\",\"source_language\":\"zh\","
            + "\"target_language\":\"en\",\"timestamp\":\"1704067200000\"}");
        success(niutrans);
        if (!niutrans.contains("\"body_utf8\":")) throw new AssertionError("NiuTrans descriptor lacks its form body");
        String parsed = NativeClient.parseCustomTranslationResponse(
            "{\"code\":200,\"data\":\" hello \"}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        if (!parsed.contains("\"value\":\"hello\"")) throw new AssertionError(parsed);
        String batch = NativeClient.parseTencentTranslationResponse(
            "{\"Response\":{\"TargetTextList\":[\"hello\",\"\"]}}".getBytes(java.nio.charset.StandardCharsets.UTF_8), 2);
        if (!batch.contains("\"value\":[\"hello\",null]")) throw new AssertionError(batch);
        if (!NativeClient.parseTencentTranslationResponse(new byte[] {'{', '}'}, 11).contains("\"ok\":false"))
            throw new AssertionError("Tencent parser accepted an oversized batch");
        if (!NativeClient.customTranslationReplyFailed("{\"code\":500}".getBytes(java.nio.charset.StandardCharsets.UTF_8))
                || NativeClient.customTranslationReplyFailed("{\"code\":200,\"data\":\"\"}".getBytes(java.nio.charset.StandardCharsets.UTF_8)))
            throw new AssertionError("custom reply failure was misread");
        if (!NativeClient.niutransTranslationReplyFailed("{\"errorCode\":\"13001\"}".getBytes(java.nio.charset.StandardCharsets.UTF_8))
                || NativeClient.niutransTranslationReplyFailed("{\"tgtText\":\"\"}".getBytes(java.nio.charset.StandardCharsets.UTF_8)))
            throw new AssertionError("NiuTrans reply failure was misread");
        String issue = NativeClient.translationEndpointIssue("http://192.168.1.2/translate");
        if (!issue.contains("\"accepted\":false") || issue.contains("\"issue\":\"\""))
            throw new AssertionError("a LAN http endpoint was accepted: " + issue);
        String test = NativeClient.testApiCredential("{\"service\":\"translation.custom\",\"config\":{}}");
        // 不完整的配置在联网之前就得到回答。
        success(test);
        if (!test.contains("请填写有效的翻译服务凭据和接口地址。")) throw new AssertionError(test);
        System.out.println("JNI consumer: translation service descriptors, parsers and settings checks passed");
    }
}
