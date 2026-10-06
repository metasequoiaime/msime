package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.provider.Settings;
import android.widget.LinearLayout;
import app.msime.android.R;

/**
 * 使用帮助：怎么启用、怎么打字、出问题了怎么办。内容从原来的 HelpActivity 原样搬过来，排成详情页的分组卡片。
 *
 * <p>步骤是 Android 自己的：照搬 Apple 那边的「设置 → 通用 → 键盘」和长按地球键，只会把人带到这台手机上不存在的地方。iOS 的「需要完全访问权限」一节在这里没有对应物，不写。
 */
public final class HelpPage extends DetailPage {
    private static final String DOCUMENTATION = "https://msime.app/docs/";

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();
        String app = context.getString(R.string.app_name);

        GroupCard enable = GroupCard.add(column, "启用键盘").withDividers(Ui.ROW_PADDING_H);
        enable.value("1. 打开键盘设置", "前往「设置 → 系统 → 语言和输入法 → 屏幕键盘」，或直接用下面那一行。", null);
        enable.value("2. 启用" + app, "在屏幕键盘列表里打开" + app + "的开关，按提示确认。", null);
        enable.value("3. 切换并开始输入", "点任意输入框，再点右下角的键盘图标选择" + app + "。", null);
        enable.nav("打开系统键盘设置", "直接跳到系统里对应的位置", null,
            () -> open(context, new Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)));

        GroupCard typing = GroupCard.add(column, "打字").withDividers(Ui.ROW_PADDING_H);
        typing.value("选择候选词", "点候选栏里的词上屏。候选多于一行时，点右端的箭头展开整页。", null);
        typing.value("换一种输入方案", "点键盘上的「拼26」那类角标，在输入方案里选全拼、双拼、五笔等。", null);
        typing.value("换皮肤与布局", "键盘工具条上的衣架换皮肤，滑块调按键高度和间距。", null);

        GroupCard trouble = GroupCard.add(column, "遇到问题").withDividers(Ui.ROW_PADDING_H);
        trouble.value("键盘里没有水杉", "回到上面的启用步骤确认开关已打开；开过仍看不到时，点输入框右下角的键盘图标翻一下列表。", null);
        trouble.value("社区连不上", "社区目录不需要登录就能读。读不出来时多为网络或服务端限流，过一会儿再试。", null);
        trouble.value("更新后行为变了", "词库随版本更新。确认装的是最新版本，或在发布页查看这一版改了什么。", null);

        GroupCard more = GroupCard.add(column, "更多").withDividers(Ui.ROW_PADDING_H);
        more.nav("完整文档", "msime.app，在浏览器里打开", null, () -> AboutPage.openLink(context, DOCUMENTATION));
        more.nav("反馈问题与建议", "在应用内写，可以附上截图和诊断信息", null,
            () -> SettingsNavigator.open(context, PageId.FEEDBACK, null));
    }

    private static void open(Context context, Intent intent) {
        try {
            context.startActivity(intent);
        } catch (RuntimeException unavailable) {
            MsToast.show(context, "这台设备上打不开这个设置");
        }
    }
}
