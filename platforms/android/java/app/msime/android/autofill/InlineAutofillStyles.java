package app.msime.android;

import android.os.Bundle;
import java.lang.reflect.InvocationTargetException;

/** 纯 SDK 宿主与 Gradle 提供的 AndroidX 样式适配之间的装配入口。 */
final class InlineAutofillStyles {
    private InlineAutofillStyles() { }

    static Bundle create(Bundle rendererExtras, int foreground, int background) {
        try {
            return (Bundle) Class.forName("app.msime.android.AndroidXInlineAutofillStyles")
                .getMethod("create", Bundle.class, int.class, int.class)
                .invoke(null, rendererExtras, foreground, background);
        } catch (ClassNotFoundException | NoSuchMethodException | IllegalAccessException
                | InvocationTargetException | SecurityException | LinkageError unavailable) {
            // 独立 SDK 门禁不解析 AAR；正式包中由 Gradle 与设备验收保证适配存在。
            return null;
        }
    }
}
