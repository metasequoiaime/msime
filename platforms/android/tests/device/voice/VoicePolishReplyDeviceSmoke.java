package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import app.msime.android.VoicePolisher;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用 Android 的真实 JSON 实现核对润色回复，不访问网络或用户资料。 */
public final class VoicePolishReplyDeviceSmoke extends Instrumentation {
    private static final String POLISHED = "synthetic polished text";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            // 直接调用产品的私有解析入口，不为测试扩大生产 API。
            Method parser = VoicePolisher.class.getDeclaredMethod("content", String.class);
            parser.setAccessible(true);
            Object[][] cases = {
                {"length", "length", null, ""},
                {"filter", "content_filter", null, ""},
                {"error", null, new JSONObject().put("message", "synthetic failure"), ""},
                {"empty-error", null, new JSONObject(), ""},
                {"false-error", null, false, ""},
                {"zero-error", null, 0, ""},
                {"empty-string-error", null, "", ""},
                {"stop", "stop", null, POLISHED},
                {"legacy", null, null, POLISHED},
                {"null-finish", JSONObject.NULL, null, POLISHED},
                {"null-error", "stop", JSONObject.NULL, POLISHED},
            };
            List<String> failures = new ArrayList<>();
            for (Object[] item : cases) {
                JSONObject choice = new JSONObject().put("message",
                    new JSONObject().put("content", POLISHED));
                if (item[1] != null) choice.put("finish_reason", item[1]);
                JSONObject document = new JSONObject().put("choices", new JSONArray().put(choice));
                if (item[2] != null) document.put("error", item[2]);
                check(parser, document.toString(), (String) item[3], (String) item[0], failures);
            }
            check(parser, "{\"choices\":[]}", "", "empty-choices", failures);
            if (!failures.isEmpty()) {
                result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: voice polish replies: "
                    + String.join(", ", failures) + "\n");
                finish(Activity.RESULT_CANCELED, result);
                return;
            }
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: 12 voice polish reply cases\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            // 只输出异常类别，不输出服务正文或请求数据。
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: voice polish replies ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private static void check(Method parser, String document, String expected, String name,
                              List<String> failures) throws Exception {
        if (!expected.equals(parser.invoke(null, document))) failures.add(name);
    }
}
