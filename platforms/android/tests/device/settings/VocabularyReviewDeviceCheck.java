package app.msime.android.test;

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
 * <p>复用设置 instrumentation 的显式专项（`SettingsDeviceSmoke -e vocabulary true`）：Android 35 每个清单最多 100 项 instrumentation，测试包已经到了上限。这是 AGENTS.md 要求在模拟器上跑过的文件读写：解包走应用私有目录下的暂存目录和原子改名，共享层在那里加文件锁、读词书、写进度，JVM 冒烟和 CI 的编译都碰不到。进度写在测试自己建的临时状态目录里，不碰产品的复习进度，跑完删掉。
 */
final class VocabularyReviewDeviceCheck {
    /** 与 resources/wordbook.lock.json 的 books 相同。 */
    private static final String[] BOOKS = {"zk", "gk", "cet4", "cet6", "ky", "ielts", "toefl", "gre"};

    private final DeviceSmoke test;

    VocabularyReviewDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        File root = test.getTargetContext().getFilesDir();
        File state = new File(test.getTargetContext().getCacheDir(), "vocabulary-review-smoke");
        try {
            test.stage = "wordbooks extracted";
            File wordbooks = new File(root, "bootstrap/wordbooks");
            for (String id : BOOKS) requireFile(new File(wordbooks, id + ".json"));
            requireFile(new File(wordbooks, "wordbooks-NOTICE.txt"));
            requireFile(new File(wordbooks, ".package"));

            deleteTree(state);
            require(state.mkdirs(), "临时状态目录建不出来");
            String resources = new File(root, "bootstrap").getAbsolutePath();

            test.stage = "bundled books listed";
            JSONObject loaded = call(state, resources, new JSONObject().put("operation", "load"));
            JSONArray listed = loaded.getJSONArray("wordbooks");
            for (String id : BOOKS) require(builtinTotal(listed, id) > 0, "内置词书 " + id + " 没有列出或没有词");

            test.stage = "select CET-4";
            JSONObject selected = call(state, resources, new JSONObject().put("operation", "set_settings")
                .put("wordbook", "cet4").put("new_per_day", 20)
                .put("session_limit", loaded.getJSONObject("settings").getInt("sessionLimit")));
            require("cet4".equals(selected.getJSONObject("settings").getString("wordbook")), "没有选中 CET-4");
            JSONArray queue = selected.getJSONArray("queue");
            require(queue.length() > 0, "选中 CET-4 后没有卡片");

            test.stage = "answer a card";
            String word = queue.getJSONObject(0).getString("word");
            JSONObject answered = call(state, resources, new JSONObject().put("operation", "answer")
                .put("wordbook", "cet4").put("word", word).put("known", true));
            require(answered.getInt("answeredToday") == 1, "作答后「已完成」不是 1");
            // 答「认识」的新词排到以后的某一天，今天的队列少一张。
            require(answered.getJSONArray("queue").length() == queue.length() - 1, "作答后队列没有少一张");

            test.stage = "old card answers its source book";
            JSONObject first = call(state, resources, new JSONObject().put("operation", "import")
                .put("name", "合成词书甲").put("text", "alpha,合成释义甲\n"));
            String firstId = first.getJSONObject("settings").getString("wordbook");
            JSONObject second = call(state, resources, new JSONObject().put("operation", "import")
                .put("name", "合成词书乙").put("text", "alpha,合成释义乙\n"));
            String secondId = second.getJSONObject("settings").getString("wordbook");
            require(!rawCall(state, resources, new JSONObject().put("operation", "answer")
                .put("word", "alpha").put("known", true)).optBoolean("ok", true), "缺来源的作答被接受");
            JSONObject oldAnswer = call(state, resources, new JSONObject().put("operation", "answer")
                .put("wordbook", firstId).put("word", "alpha").put("known", true));
            require(secondId.equals(oldAnswer.getJSONObject("settings").getString("wordbook")), "旧卡作答改了全局选择");
            require(oldAnswer.getJSONArray("queue").getJSONObject(0).getString("meaning").equals("合成释义乙"), "乙书进度被旧卡消耗");
            require(oldAnswer.getInt("answeredToday") == 2, "旧卡作答没有只计一次");
            JSONObject original = call(state, resources, new JSONObject().put("operation", "set_settings")
                .put("wordbook", firstId).put("new_per_day", 20).put("session_limit", 200));
            require(original.getJSONArray("queue").length() == 0, "旧卡没有记回甲书");
            call(state, resources, new JSONObject().put("operation", "remove").put("wordbook", firstId));
            call(state, resources, new JSONObject().put("operation", "set_settings")
                .put("wordbook", secondId).put("new_per_day", 20).put("session_limit", 200));
            require(!rawCall(state, resources, new JSONObject().put("operation", "answer")
                .put("wordbook", firstId).put("word", "alpha").put("known", true)).optBoolean("ok", true), "已删书的旧卡被接受");
            JSONObject unchanged = call(state, resources, new JSONObject().put("operation", "load"));
            require(unchanged.getInt("answeredToday") == 2 && unchanged.getJSONArray("queue").length() == 1,
                "拒绝旧卡后乙书或每日计数改变");

            test.stage = "bundled book is not removable";
            require(!rawCall(state, resources, new JSONObject().put("operation", "remove").put("wordbook", "cet4"))
                .optBoolean("ok", true), "内置词书被删掉了");
        } finally {
            // 临时目录在应用缓存里，删不掉也不影响产品。
            deleteTree(state);
        }
    }

    private JSONObject call(File state, String resources, JSONObject action) throws Exception {
        JSONObject root = rawCall(state, resources, action);
        require(root.optBoolean("ok", false), "共享层报错：" + root.optString("error"));
        return root.getJSONObject("value");
    }

    private JSONObject rawCall(File state, String resources, JSONObject action) throws Exception {
        Class<?> bridge = test.getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
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
