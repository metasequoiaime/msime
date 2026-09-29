package app.msime.client.home;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.content.ContextCompat;
import app.msime.client.R;
import com.google.android.material.appbar.MaterialToolbar;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;

/**
 * 在电脑上，也用水杉：选系统，看步骤，打开发布页。
 *
 * <p>The three platforms and their steps are the Apple app's `DesktopPlatform`, word for word. The
 * release page is the same one for all three -- the selector changes the guidance, not where the
 * installers come from.
 */
public final class DesktopDownloadActivity extends AppCompatActivity {
    private static final String RELEASES = "https://github.com/metasequoiaime/msime/releases";

    private enum Platform {
        MACOS("macOS", new String[] {
            "在 Mac 上打开下载页，选择适合你的 macOS 安装包。",
            "按照发布页说明完成安装。",
            "在系统设置的键盘输入法中添加水杉输入法，再切换使用。"}),
        WINDOWS("Windows", new String[] {
            "在 Windows 电脑上打开下载页，选择与你的系统架构匹配的安装包。",
            "运行安装程序，按发布页说明完成安装。",
            "使用 Win + 空格切换到水杉输入法。"}),
        LINUX("Linux", new String[] {
            "在 Linux 电脑上打开发布页，查看适用发行版与依赖要求。",
            "按照项目安装说明配置 IBus 和水杉输入法。",
            "在系统输入源中添加水杉输入法，按说明重新登录后使用。"});

        private final String title;
        private final String[] steps;

        Platform(String title, String[] steps) {
            this.title = title;
            this.steps = steps;
        }
    }

    private LinearLayout steps;

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        setContentView(R.layout.activity_desktop_download);
        MaterialToolbar bar = findViewById(R.id.desktop_bar);
        bar.setNavigationOnClickListener(ignored -> finish());
        steps = findViewById(R.id.desktop_steps);

        // The hero shows where the installers live without the scheme, the way the design prints its link.
        ((TextView) findViewById(R.id.desktop_link)).setText(RELEASES.replaceFirst("^https://", ""));

        MaterialButtonToggleGroup platforms = findViewById(R.id.desktop_platforms);
        Platform[] values = Platform.values();
        int[] segments = new int[values.length];
        for (int index = 0; index < values.length; index++) {
            MaterialButton segment = (MaterialButton) getLayoutInflater()
                .inflate(R.layout.item_segment, platforms, false);
            segment.setId(View.generateViewId());
            segment.setText(values[index].title);
            platforms.addView(segment);
            segments[index] = segment.getId();
        }
        platforms.check(segments[0]);
        platforms.addOnButtonCheckedListener((group, id, checked) -> {
            if (!checked) return;
            for (int index = 0; index < segments.length; index++) {
                if (segments[index] == id) showSteps(values[index]);
            }
        });

        MaterialButton open = findViewById(R.id.desktop_open);
        open.setOnClickListener(ignored -> {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(RELEASES)));
            } catch (RuntimeException error) {
                // 没有浏览器就靠下面那颗复制按钮，别在这里报一个用户处理不了的错。
            }
        });
        MaterialButton copy = findViewById(R.id.desktop_copy);
        copy.setOnClickListener(ignored -> {
            ClipboardManager clipboard = getSystemService(ClipboardManager.class);
            if (clipboard == null) return;
            clipboard.setPrimaryClip(ClipData.newPlainText("水杉发布页", RELEASES));
            copy.setText(R.string.desktop_copied);
        });

        showSteps(Platform.MACOS);
    }

    /** One numbered circle per step, in the accent, as the master draws them. */
    private void showSteps(Platform platform) {
        steps.removeAllViews();
        ((TextView) findViewById(R.id.desktop_steps_title))
            .setText(platform.title + " 安装指南");
        float density = getResources().getDisplayMetrics().density;
        for (int index = 0; index < platform.steps.length; index++) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.HORIZONTAL);
            LinearLayout.LayoutParams rowParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
            rowParams.topMargin = Math.round(16 * density);
            row.setLayoutParams(rowParams);

            TextView number = new TextView(this);
            number.setText(String.valueOf(index + 1));
            number.setGravity(Gravity.CENTER);
            number.setTextSize(14);
            number.setTypeface(number.getTypeface(), android.graphics.Typeface.BOLD);
            number.setTextColor(ContextCompat.getColor(this, R.color.forest));
            android.graphics.drawable.GradientDrawable disc =
                new android.graphics.drawable.GradientDrawable();
            disc.setShape(android.graphics.drawable.GradientDrawable.OVAL);
            disc.setColor(ContextCompat.getColor(this, R.color.badge_field));
            number.setBackground(disc);
            int size = Math.round(28 * density);
            row.addView(number, new LinearLayout.LayoutParams(size, size));

            TextView text = new TextView(this);
            text.setText(platform.steps[index]);
            text.setTextSize(14);
            text.setTextColor(ContextCompat.getColor(this, R.color.ink));
            LinearLayout.LayoutParams textParams = new LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1);
            textParams.leftMargin = Math.round(12 * density);
            row.addView(text, textParams);

            steps.addView(row);
        }
        steps.setContentDescription(platform.title + " 安装指南，共 " + platform.steps.length + " 步");
    }

    @Override protected void onStart() {
        super.onStart();
        // 回到这一页时把复制按钮的文案还原，否则它会一直停在「链接已复制」。
        View copy = findViewById(R.id.desktop_copy);
        if (copy instanceof MaterialButton button) button.setText(R.string.desktop_copy_link);
    }
}
