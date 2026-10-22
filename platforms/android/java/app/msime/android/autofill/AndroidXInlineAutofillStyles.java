package app.msime.android;

import android.os.Bundle;
import androidx.autofill.inline.UiVersions;
import androidx.autofill.inline.common.TextViewStyle;
import androidx.autofill.inline.common.ViewStyle;
import androidx.autofill.inline.v1.InlineSuggestionUi;

/** 使用官方版本化协议生成远程填充界面样式；不解析填充内容。 */
public final class AndroidXInlineAutofillStyles {
    private AndroidXInlineAutofillStyles() { }

    @android.annotation.TargetApi(30)
    public static Bundle create(Bundle rendererExtras, int foreground, int background) {
        if (!UiVersions.getVersions(rendererExtras).contains(UiVersions.INLINE_UI_VERSION_1)) return null;
        return UiVersions.newStylesBuilder().addStyle(InlineSuggestionUi.newStyleBuilder()
            .setChipStyle(new ViewStyle.Builder().setBackgroundColor(background).build())
            .setTitleStyle(new TextViewStyle.Builder().setTextColor(foreground).setTextSize(14).build())
            .setSubtitleStyle(new TextViewStyle.Builder().setTextColor(foreground).setTextSize(12).build())
            .build()).build();
    }
}
