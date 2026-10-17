package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;

/** 账号数据汇总的 malformed 响应回归。 */
public final class DeviceDataSummaryDeviceSmoke extends Instrumentation {
    private static final String TOKEN = "a".repeat(64);

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    private static void missingSectionsAreRejected() throws Exception {
        CloudApi cloud = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null, "{\"bytes\":0}".getBytes()),
            ignored -> TOKEN, ignored -> "");
        DeviceDataApi api = new DeviceDataApi(cloud, ignored -> TOKEN,
            (path, token, output) -> { throw new AssertionError("no export expected"); });
        boolean rejected = false;
        try {
            api.dataSummary();
        } catch (CloudApi.Failure failure) {
            rejected = failure.status == 500 && "invalid_response".equals(failure.code);
        }
        check(rejected, "missing data sections must not be treated as empty");
    }

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            missingSectionsAreRejected();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: malformed account data summary\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: malformed account data summary ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}
