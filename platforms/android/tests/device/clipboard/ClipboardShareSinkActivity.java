package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.widget.TextView;

/** 系统分享面板中的本地合成接收器，不发送网络请求。 */
public final class ClipboardShareSinkActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        TextView value = new TextView(this);
        value.setContentDescription("msime-test-shared-text");
        value.setText(getIntent().getCharSequenceExtra(Intent.EXTRA_TEXT));
        setContentView(value);
    }
}
