package app.msime.android;

import android.content.res.Configuration;

/** 从配置回调的 uiMode 判定深浅，其他 UI 类型位不能影响夜间模式。 */
public final class DimensionPolicySmoke {
    public static void main(String[] args) {
        if (!DimensionPolicy.isNight(Configuration.UI_MODE_TYPE_NORMAL | Configuration.UI_MODE_NIGHT_YES)
                || !DimensionPolicy.isNight(Configuration.UI_MODE_TYPE_CAR | Configuration.UI_MODE_NIGHT_YES)
                || DimensionPolicy.isNight(Configuration.UI_MODE_TYPE_NORMAL | Configuration.UI_MODE_NIGHT_NO)
                || DimensionPolicy.isNight(Configuration.UI_MODE_TYPE_CAR | Configuration.UI_MODE_NIGHT_NO)
                || DimensionPolicy.isNight(Configuration.UI_MODE_NIGHT_UNDEFINED))
            throw new AssertionError("Configuration night-mode mask is incorrect");
        System.out.println("Android configuration night-mode policy passed");
    }
}
