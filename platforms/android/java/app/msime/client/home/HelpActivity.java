package app.msime.client.home;

import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.provider.Settings;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.content.ContextCompat;
import app.msime.client.R;
import com.google.android.material.appbar.MaterialToolbar;

/**
 * 使用帮助：怎么启用、怎么打字、出问题了怎么办。
 *
 * <p>Sectioned like the Apple app's `HelpView`, but the steps are Android's own. Copying that copy
 * would send people to 设置 → 通用 → 键盘 and tell them to long-press a globe key for a picker that
 * is not where it says — instructions for a phone they are not holding are worse than none.
 *
 * <p>The 需要完全访问权限 section has no counterpart here at all: that permission is an iOS keyboard
 * extension's, and on Android the statistics and the handwriting model need nothing of the sort.
 */
public final class HelpActivity extends AppCompatActivity {
    private static final String DOCUMENTATION = "https://msime.app/docs/";

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        setContentView(R.layout.activity_help);
        MaterialToolbar bar = findViewById(R.id.help_bar);
        bar.setNavigationOnClickListener(ignored -> finish());
        LinearLayout column = findViewById(R.id.help_column);

        section(column, "启用键盘");
        LinearLayout enable = card(column);
        item(enable, "1. 打开键盘设置", "前往「设置 → 系统 → 语言和输入法 → 屏幕键盘」，或直接用下面那一行。");
        item(enable, "2. 启用水杉输入法", "在屏幕键盘列表里打开水杉输入法的开关，按提示确认。");
        item(enable, "3. 切换并开始输入", "点任意输入框，再点右下角的键盘图标选择水杉输入法。");
        action(enable, "打开系统键盘设置", "直接跳到系统里对应的位置", R.drawable.ic_feature_system,
            () -> open(new Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)));

        section(column, "打字");
        LinearLayout typing = card(column);
        item(typing, "选择候选词", "点候选栏里的词上屏。候选多于一行时，点右端的箭头展开整页。");
        item(typing, "换一种输入方案", "点键盘上的「拼26」那类角标，在输入方案里选全拼、双拼、五笔等。");
        item(typing, "换皮肤与布局", "键盘工具条上的衣架换皮肤，滑块调按键高度和间距。");

        section(column, "遇到问题");
        LinearLayout trouble = card(column);
        item(trouble, "键盘里没有水杉", "回到上面的启用步骤确认开关已打开；开过仍看不到时，点输入框右下角的键盘图标翻一下列表。");
        item(trouble, "社区连不上", "社区目录不需要登录就能读。读不出来时多为网络或服务端限流，过一会儿再试。");
        item(trouble, "更新后行为变了", "词库随版本更新。确认装的是最新版本，或在发布页查看这一版改了什么。");

        section(column, "更多");
        LinearLayout more = card(column);
        action(more, "完整文档", "msime.app，在浏览器里打开", R.drawable.ic_about_site,
            () -> open(new Intent(Intent.ACTION_VIEW, Uri.parse(DOCUMENTATION))));
        // 我的 → 帮助与反馈 lands here, so the feedback half of that row has to be here too.
        action(more, "反馈问题与建议", "在应用内写，附带版本与设备信息", R.drawable.ic_about_feedback,
            () -> open(new Intent(this, FeedbackActivity.class)));
    }

    private void open(Intent intent) {
        try {
            startActivity(intent);
        } catch (RuntimeException error) {
            // 没有应用接得住就什么也不做：这一行是个入口，不是一件非成功不可的操作。
        }
    }

    private void section(LinearLayout parent, String title) {
        TextView heading = ListRows.heading(parent, title);
        heading.setPadding(pixels(16), pixels(20), pixels(16), 0);
    }

    private LinearLayout card(LinearLayout parent) {
        com.google.android.material.card.MaterialCardView card =
            new com.google.android.material.card.MaterialCardView(this);
        card.setRadius(pixels(20));
        card.setStrokeWidth(0);
        card.setCardElevation(0);
        card.setCardBackgroundColor(ContextCompat.getColor(this, R.color.surface));
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setPadding(0, pixels(6), 0, pixels(6));
        card.addView(column);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.topMargin = pixels(8);
        parent.addView(card, params);
        return column;
    }

    /** 一条：黑体的名目，底下一行解释。 */
    private void item(LinearLayout parent, String term, String detail) {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.VERTICAL);
        TextView title = new TextView(this);
        title.setText(term);
        title.setTextSize(15);
        title.setTextColor(ContextCompat.getColor(this, R.color.ink));
        row.addView(title);
        TextView body = new TextView(this);
        body.setText(detail);
        body.setTextSize(13);
        body.setTextColor(ContextCompat.getColor(this, R.color.text_secondary));
        LinearLayout.LayoutParams bodyParams = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        bodyParams.topMargin = pixels(3);
        row.addView(body, bodyParams);
        row.setContentDescription(term + "。" + detail);
        row.setPadding(pixels(16), 0, pixels(16), 0);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.topMargin = pixels(10);
        params.bottomMargin = pixels(6);
        parent.addView(row, params);
    }

    /** A tappable row inside a card: the shared M3 row at the card's 16dp inset. */
    private void action(LinearLayout parent, String title, String value, int icon, Runnable run) {
        View row = ListRows.add(parent, icon, title, value, run);
        row.setPaddingRelative(pixels(16), row.getPaddingTop(), pixels(16), row.getPaddingBottom());
        row.setContentDescription(title + "，" + value);
    }

    private int pixels(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
