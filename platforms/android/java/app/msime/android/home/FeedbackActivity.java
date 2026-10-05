package app.msime.android.home;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.widget.EditText;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import app.msime.android.R;
import app.msime.android.policy.FeedbackBodyPolicy;
import com.google.android.material.appbar.MaterialToolbar;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;

/**
 * 反馈问题与建议：写一段，连同版本和机型一起带走。
 *
 * <p>The same shape as the Apple app's `FeedbackView` -- a kind, a description, the environment
 * shown rather than collected silently, and the two ways out: copy the whole thing, or open a
 * prefilled issue. The environment is Android's here, and it is on screen because a report that
 * quietly attaches facts about someone's device should at least show them which facts.
 */
public final class FeedbackActivity extends AppCompatActivity {
    private static final String[] KINDS = {"功能异常", "候选词不对", "功能建议", "其他"};
    private static final String ISSUES = "https://github.com/metasequoiaime/msime/issues/new";
    // These two are the same strings the macOS, iOS and shared feedback pages carry. There is no
    // shared constant for them, which is why they have now been written out four times; the fifth
    // copy is guarded by scripts/test-support-channels.py rather than left to be noticed.
    private static final String QQ_GROUP = "829919142";
    private static final String TELEGRAM_URL = "https://t.me/msimegroup";
    private String kind = KINDS[0];
    private EditText detail;

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        setContentView(R.layout.activity_feedback);
        MaterialToolbar bar = findViewById(R.id.feedback_bar);
        bar.setNavigationOnClickListener(ignored -> finish());

        ChipGroup kinds = findViewById(R.id.feedback_kinds);
        for (String value : KINDS) {
            Chip chip = new Chip(this);
            chip.setText(value);
            chip.setCheckable(true);
            chip.setId(android.view.View.generateViewId());
            chip.setChecked(value.equals(kind));
            chip.setOnClickListener(ignored -> kind = value);
            kinds.addView(chip);
        }

        detail = findViewById(R.id.feedback_detail);
        ((TextView) findViewById(R.id.feedback_environment)).setText(environment());

        MaterialButton copy = findViewById(R.id.feedback_copy);
        copy.setOnClickListener(ignored -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            if (clipboard == null) return;
            clipboard.setPrimaryClip(ClipData.newPlainText("水杉反馈", report()));
            copy.setText(R.string.feedback_copied);
        });

        findViewById(R.id.feedback_submit).setOnClickListener(ignored -> submit());

        // The two channels every other host lists on its own feedback screen. This was the only
        // feedback surface in the app and it named neither, so an Android user's only route was the
        // GitHub issue form - the slowest of the three and the wrong one for a question.
        MaterialButton group = findViewById(R.id.feedback_qq);
        group.setOnClickListener(ignored -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            if (clipboard == null) return;
            clipboard.setPrimaryClip(ClipData.newPlainText("QQ 群号", QQ_GROUP));
            group.setText(R.string.feedback_qq_copied);
        });
        findViewById(R.id.feedback_telegram).setOnClickListener(ignored -> openTelegram());
    }

    /** No handler for a t.me link is an ordinary state on a device without Telegram installed. */
    private void openTelegram() {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, android.net.Uri.parse(TELEGRAM_URL)));
        } catch (android.content.ActivityNotFoundException error) {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            if (clipboard != null) {
                clipboard.setPrimaryClip(ClipData.newPlainText("Telegram", TELEGRAM_URL));
            }
            android.widget.Toast.makeText(this, "已复制群组链接", android.widget.Toast.LENGTH_SHORT)
                .show();
        }
    }

    @Override protected void onStart() {
        super.onStart();
        ((MaterialButton) findViewById(R.id.feedback_copy)).setText(R.string.feedback_copy);
    }

    /** 版本、系统和机型——附上去的就是屏幕上显示的这三行，没有别的。 */
    private String environment() {
        String version = "开发构建";
        String build = "-";
        try {
            PackageInfo info = getPackageManager().getPackageInfo(getPackageName(), 0);
            if (info.versionName != null) version = info.versionName;
            build = String.valueOf(Build.VERSION.SDK_INT >= 28
                ? info.getLongVersionCode() : info.versionCode);
        } catch (PackageManager.NameNotFoundException error) {
            // 查不到自己的包就用上面那两个占位，别把一个假版本号写进别人的问题单。
        }
        return getString(R.string.app_name) + " " + version + "（构建 " + build + "）\n"
            + "Android " + Build.VERSION.RELEASE + "（API " + Build.VERSION.SDK_INT + "）\n"
            + Build.MANUFACTURER + " " + Build.MODEL;
    }

    private String report() {
        String text = detail == null ? "" : detail.getText().toString();
        return "### 类型\n" + kind + "\n\n### 描述\n" + text + "\n\n### 环境\n" + environment() + "\n";
    }

    private void submit() {
        String body = FeedbackBodyPolicy.clip(report());
        Uri url = Uri.parse(ISSUES).buildUpon()
            .appendQueryParameter("title", kind)
            .appendQueryParameter("body", body)
            .build();
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, url));
        } catch (RuntimeException error) {
            // 打不开浏览器时还有「复制报告」那条路，不在这里报一个用户处理不了的错。
        }
    }
}
