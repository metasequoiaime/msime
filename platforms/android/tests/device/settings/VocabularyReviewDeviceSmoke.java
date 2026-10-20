package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.time.LocalDate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 背单词内置词书的设备验收：Bootstrap 把 APK 里的八本词书和许可声明解到 `files/bootstrap/wordbooks`，设置页经真实 JNI（`NativeClient.vocabularyReview`）以 `files/bootstrap` 为资源根读到它们，选 CET-4 后发出卡片，作答一次计数随之变化。
 *
 * <p>这是 AGENTS.md 要求在模拟器上跑过的文件读写：解包走应用私有目录下的暂存目录和原子改名，共享层在那里加文件锁、读词书、写进度，JVM 冒烟和 CI 的编译都碰不到。进度写在测试自己建的临时状态目录里，不碰产品的复习进度，跑完删掉。
 */
public final class VocabularyReviewDeviceSmoke extends Instrumentation {
    /** 与 resources/wordbook.lock.json 的 books 相同。 */
    private static final String[] BOOKS = {"zk", "gk", "cet4", "cet6", "ky", "ielts", "toefl", "gre"};

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        String stage = "launch";
        File state = new File(getTargetContext().getCacheDir(), "vocabulary-review-smoke");
        try {
            File root = getTargetContext().getFilesDir();
            stage = "wordbooks extracted";
            File wordbooks = new File(root, "bootstrap/wordbooks");
            for (String id : BOOKS) requireFile(new File(wordbooks, id + ".json"));
            requireFile(new File(wordbooks, "wordbooks-NOTICE.txt"));
            requireFile(new File(wordbooks, ".package"));

            deleteTree(state);
            require(state.mkdirs(), "临时状态目录建不出来");
            String resources = new File(root, "bootstrap").getAbsolutePath();

            stage = "bundled books listed";
            JSONObject loaded = call(state, resources, new JSONObject().put("operation", "load"));
            JSONArray listed = loaded.getJSONArray("wordbooks");
            for (String id : BOOKS) require(builtinTotal(listed, id) > 0, "内置词书 " + id + " 没有列出或没有词");

            stage = "select CET-4";
            JSONObject selected = call(state, resources, new JSONObject().put("operation", "set_settings")
                .put("wordbook", "cet4").put("new_per_day", 20)
                .put("session_limit", loaded.getJSONObject("settings").getInt("sessionLimit")));
            require("cet4".equals(selected.getJSONObject("settings").getString("wordbook")), "没有选中 CET-4");
            JSONArray queue = selected.getJSONArray("queue");
            require(queue.length() > 0, "选中 CET-4 后没有卡片");

            stage = "answer a card";
            String word = queue.getJSONObject(0).getString("word");
            JSONObject answered = call(state, resources, new JSONObject().put("operation", "answer")
                .put("word", word).put("known", true));
            require(answered.getInt("answeredToday") == 1, "作答后「已完成」不是 1");
            // 答「认识」的新词排到以后的某一天，今天的队列少一张。
            require(answered.getJSONArray("queue").length() == queue.length() - 1, "作答后队列没有少一张");

            stage = "bundled book is not removable";
            require(!rawCall(state, resources, new JSONObject().put("operation", "remove").put("wordbook", "cet4"))
                .optBoolean("ok", true), "内置词书被删掉了");

            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: bundled wordbooks extracted, listed through JNI and answered\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_VOCABULARY_REVIEW_FAILED at " + stage + ": "
                + error.getClass().getSimpleName() + " " + error.getMessage() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            try {
                deleteTree(state);
            } catch (Exception ignored) {
                // 临时目录在应用缓存里，删不掉也不影响产品。
            }
        }
    }

    private JSONObject call(File state, String resources, JSONObject action) throws Exception {
        JSONObject root = rawCall(state, resources, action);
        require(root.optBoolean("ok", false), "共享层报错：" + root.optString("error"));
        return root.getJSONObject("value");
    }

    private JSONObject rawCall(File state, String resources, JSONObject action) throws Exception {
        Class<?> bridge = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        Method review = bridge.getMethod("vocabularyReview", String.class);
        String request = new JSONObject().put("directory", state.getAbsolutePath()).put("resources", resources)
            .put("day", LocalDate.now().toString()).put("action", action).toString();
        return new JSONObject((String) review.invoke(null, request));
    }

    private static int builtinTotal(JSONArray books, String id) throws Exception {
        for (int index = 0; index < books.length(); index++) {
            JSONObject book = books.getJSONObject(index);
            if (id.equals(book.getString("id"))) return book.getBoolean("builtin") ? book.getInt("total") : -1;
        }
        return -1;
    }

    private static void requireFile(File file) {
        require(Files.isRegularFile(file.toPath(), LinkOption.NOFOLLOW_LINKS), "缺少 " + file.getName());
    }

    private static void deleteTree(File file) throws Exception {
        if (Files.isDirectory(file.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            File[] children = file.listFiles();
            for (File child : children == null ? new File[0] : children) deleteTree(child);
        }
        Files.deleteIfExists(file.toPath());
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
