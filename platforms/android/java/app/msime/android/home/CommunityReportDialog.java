package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.RadioButton;
import android.widget.RadioGroup;
import androidx.appcompat.app.AlertDialog;
import androidx.fragment.app.Fragment;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import com.google.android.material.textfield.TextInputEditText;
import com.google.android.material.textfield.TextInputLayout;

/**
 * 举报一款社区作品：固定的几个原因选一个，加可选的补充说明，用这台设备的账号发出（匿名账号也算）。
 *
 * <p>原先是社区 tab 自己的私有方法；社区内容并进各功能页以后，完整列表和 AI 页的回复模板都要能举报，所以抽成共用的一处。
 */
final class CommunityReportDialog {
    private CommunityReportDialog() {}

    /** 在工作线程上发出举报，返回要显示的失败说明；受理时是空字符串。 */
    interface Reporter {
        String report(Context context, String reason, String detail);
    }

    static void show(Fragment fragment, CommunityCatalog.Item item) {
        show(fragment, item.name(), (context, reason, detail) -> new CommunityCatalog(context).report(item, reason, detail));
    }

    /** 举报一个不经 {@link CommunityCatalog} 列出的作品（社区插件）：原因和说明的规则相同，发出举报的那一步由 `reporter` 决定。 */
    static void show(Fragment fragment, String name, Reporter reporter) {
        Context context = fragment.requireContext();
        int padding = DimensionPolicy.pixels(context, 20);
        LinearLayout form = LayoutPolicy.column(context);
        ViewPolicy.setPadding(form, padding, DimensionPolicy.pixels(context, 8), padding, 0);
        RadioGroup reasons = new RadioGroup(context);
        for (String reason : CommunityRequest.REPORT_REASONS) {
            RadioButton choice = new RadioButton(context);
            choice.setId(View.generateViewId());
            choice.setText(reason);
            choice.setTag(reason);
            reasons.addView(choice);
        }
        form.addView(reasons);
        TextInputLayout detailField = new TextInputLayout(context);
        detailField.setHint("补充说明（可选）");
        detailField.setCounterEnabled(true);
        detailField.setCounterMaxLength(CommunityRequest.MAX_REPORT_DETAIL);
        TextInputEditText detail = new TextInputEditText(detailField.getContext());
        ViewPolicy.setMaxLines(detail, 4);
        detailField.addView(detail);
        form.addView(detailField);

        AlertDialog dialog = new MaterialAlertDialogBuilder(context)
            .setTitle("举报「" + name + "」")
            .setView(form)
            .setNegativeButton("取消", null)
            .setPositiveButton("提交", null)
            .create();
        dialog.setOnShowListener(ignored -> {
            View submit = dialog.getButton(AlertDialog.BUTTON_POSITIVE);
            ViewPolicy.setEnabled(submit, false);
            reasons.setOnCheckedChangeListener((group, checked) -> ViewPolicy.setEnabled(submit, checked != -1));
            ViewPolicy.bindClick(submit, () -> {
                View checked = reasons.findViewById(reasons.getCheckedRadioButtonId());
                String reason = checked == null ? "" : String.valueOf(checked.getTag());
                String text = TextPolicy.trimmed(detail.getText() == null ? null : detail.getText().toString());
                if (!CommunityRequest.validReport(reason, text)) {
                    detailField.setError("最多 " + CommunityRequest.MAX_REPORT_DETAIL + " 字");
                    return;
                }
                dialog.dismiss();
                HostTask.runNetwork(fragment, worker -> reporter.report(worker, reason, text),
                    failure -> {
                        if (fragment.getView() == null) return;
                        MsToast.show(fragment.requireContext(), failure == null || failure.isEmpty()
                            ? "已收到举报，管理员会尽快处理。" : failure);
                    });
            });
        });
        dialog.show();
    }
}
