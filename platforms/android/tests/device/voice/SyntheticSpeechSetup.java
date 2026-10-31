package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.ComponentName;
import android.content.pm.PackageManager;
import android.os.Bundle;

/** 在测试包自己的 UID 中启停合成服务，避免依赖 shell 修改组件状态的特权。 */
public final class SyntheticSpeechSetup extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        getTargetContext().getSharedPreferences("synthetic-speech", 0).edit()
            .putBoolean("hold", "true".equals(arguments.getString("hold"))).commit();
        Bundle result = new Bundle();
        int state = "true".equals(arguments.getString("enabled"))
            ? PackageManager.COMPONENT_ENABLED_STATE_ENABLED : PackageManager.COMPONENT_ENABLED_STATE_DISABLED;
        for (String service : new String[] {"SyntheticSpeechService", "LegacySyntheticSpeechService"}) {
            getTargetContext().getPackageManager().setComponentEnabledSetting(
                new ComponentName("app.msime.android.test", "app.msime.android.test." + service),
                state, PackageManager.DONT_KILL_APP);
        }
        result.putString("stream", "MSIME_SPEECH_SETUP_PASSED\n");
        finish(Activity.RESULT_OK, result);
    }
}
