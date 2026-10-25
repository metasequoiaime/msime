package app.msime.android.test;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.Intent;
import android.os.Bundle;
import android.speech.RecognizerIntent;
import android.widget.TextView;

/** 独立测试应用通过标准 Activity 契约接收合成结果，不访问模型或输入法存储。 */
public final class OfflineVoiceCallerActivity extends Activity {
    private static final int REQUEST = 9159;
    static final String REPORT = "app.msime.android.test.OFFLINE_CALLER_REPORT";
    private PendingIntent report;
    private TextView status;
    private int deliveries;

    @SuppressWarnings("deprecation") // 测试调用器在 API 28 上读取系统传递的 PendingIntent。
    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        report = getIntent().getParcelableExtra(REPORT);
        if (report == null) {
            finish();
            return;
        }
        status = new TextView(this);
        status.setText("MSIME_OFFLINE_CALLER_WAITING");
        setContentView(status);
        if (saved == null) {
            Intent request = new Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
                .setPackage("app.msime.android")
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE, "en-US")
                .putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true);
            startActivityForResult(request, REQUEST);
        }
    }

    @Override protected void onActivityResult(int request, int code, Intent data) {
        super.onActivityResult(request, code, data);
        if (request != REQUEST) return;
        deliveries++;
        var results = data == null ? null : data.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS);
        String text = results == null ? "none" : results.size() == 1 ? results.get(0) : "invalid";
        boolean hasResults = data != null && data.hasExtra(RecognizerIntent.EXTRA_RESULTS);
        status.setText("MSIME_OFFLINE_CALLER:" + deliveries + ":" + code + ":" + hasResults + ":" + text);
        // 测试能力仅由显式启动本夹具的 instrumentation 提供；产品仍只回调标准 Activity 结果。
        Intent observed = data == null ? new Intent() : new Intent(data);
        observed.putExtra("caller_deliveries", deliveries);
        try {
            report.send(this, code, observed);
        } catch (PendingIntent.CanceledException error) {
            throw new AssertionError("Caller observation expired", error);
        }
        finish();
    }
}
