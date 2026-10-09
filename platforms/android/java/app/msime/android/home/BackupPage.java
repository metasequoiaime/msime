package app.msime.android.home;

import android.net.Uri;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.time.format.DateTimeParseException;
import java.util.Locale;

/**
 * 备份与恢复（#5659）：把设置、自定义皮肤、常用语、个人词库和输入记录导出成本机的一个 zip，换手机、重装或降级后再从它恢复。全程只在本机，不经过云端，也不需要登录；文件存到用户在系统文件选择器里选的位置（SAF），不申请存储权限。输入记录能看出打字习惯，文件又是明文的，所以页面和导出结果都提醒用户妥善保管。打包与恢复在 {@link LocalBackup}，包的格式在 {@code LocalBackupPolicy}。
 *
 * <p>恢复前先整份校验（校验和、各条目的格式），再读出备份的说明让用户确认；损坏或不完整的文件不进入恢复。恢复是合并：设置按备份改写，皮肤、常用语、词和输入记录合并进来，本机已有的不会删除；设置、皮肤或常用语有一部分写不进去时整次撤销，本机保持恢复前的样子。
 */
public final class BackupPage extends DetailPage {
    private static final DateTimeFormatter DAY = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm", Locale.ROOT);

    private boolean busy;
    @Nullable private GroupCard.Row exportRow;
    @Nullable private GroupCard.Row restoreRow;

    private final ActivityResultLauncher<String> createDocument =
        registerForActivityResult(new ActivityResultContracts.CreateDocument("application/zip"), this::onExportPicked);
    private final ActivityResultLauncher<String[]> openDocument =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onRestorePicked);

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        GroupCard intro = GroupCard.add(column, null);
        intro.note("把设置、自定义皮肤、常用语、个人词库（自造词和学到的词）和输入记录（候选顺序的学习调整、固定和置顶的候选、选词次数、整句联想学到的上下文、拼写纠错习惯）导出成一个文件，存在你选的位置。换手机、重装或降级以后，从这个文件恢复。全程只在本机，不经过云端，也不需要登录。");
        intro.note(LocalBackup.PRIVACY_NOTICE);

        GroupCard backup = GroupCard.add(column, "备份");
        exportRow = backup.button("导出备份", "文件名带应用名和版本号，降级时容易找到对应的那份", "导出",
            this::startExport);

        GroupCard restore = GroupCard.add(column, "恢复");
        restoreRow = restore.button("从备份恢复", "设置按备份改写；皮肤、常用语、词和输入记录合并进来，本机已有的不会删除",
            "选择文件", this::startRestore);
        restore.footer("恢复前会先检查文件是否完整，损坏的文件不会改动本机；设置、皮肤或常用语有一部分写不进去时，这次恢复会整个撤销。备份里没有账号、AI 与翻译服务的凭据、剪贴板历史和诊断日志，不带「上传语音以改进识别」的授权和开发者选项，也没有输入统计和命名词库的分组（词库里的词随个人词库一起恢复）；自定义皮肤只有设计参数，不含照片。恢复的词在键盘空闲时陆续写入词库；输入记录也在键盘空闲时合并，本机已经学到的保留本机，各种次数取两边较多的那个。输入记录不随云同步上传。");
        refresh();
    }

    @Override public void onDestroyView() {
        exportRow = null;
        restoreRow = null;
        super.onDestroyView();
    }

    private void refresh() {
        if (exportRow != null) exportRow.setEnabled(!busy);
        if (restoreRow != null) restoreRow.setEnabled(!busy);
    }

    private void setBusy(boolean value) {
        busy = value;
        refresh();
    }

    // ---- 导出 ----

    private void startExport() {
        if (busy) return;
        createDocument.launch(LocalBackup.defaultName(requireContext()));
    }

    private void onExportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        setBusy(true);
        MsToast.show(requireContext(), "正在导出…");
        HostTask.run(this, context -> LocalBackup.export(context, uri), message -> {
            setBusy(false);
            showResult("导出备份", message == null ? "没有导出成功，请稍后重试。" : message);
        });
    }

    // ---- 恢复 ----

    private void startRestore() {
        if (busy) return;
        // 有的文件管理器给 zip 标的类型是 application/octet-stream，一并列出来，读的时候再按内容判断。
        openDocument.launch(new String[] {"application/zip", "application/x-zip-compressed", "application/octet-stream"});
    }

    private void onRestorePicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        setBusy(true);
        HostTask.run(this, context -> LocalBackup.prepare(context, uri), prepared -> {
            setBusy(false);
            if (prepared == null) {
                showResult("从备份恢复", "读不出这份文件，请确认选的是导出的备份 .zip 文件。");
                return;
            }
            LocalBackup.Preview preview = prepared.preview();
            switch (preview.compatibility()) {
                case NOT_A_BACKUP -> showResult("从备份恢复", "这不是水杉输入法导出的备份文件。");
                case NEWER_FORMAT -> showResult("从备份恢复", "这份备份来自更新的版本，请先把应用更新到最新版再恢复。");
                case DAMAGED -> showResult("从备份恢复", "这份备份文件已损坏或不完整，没有恢复，本机没有任何改动。请换一份备份，或重新导出后再试。");
                case OK -> confirmRestore(prepared);
            }
        });
    }

    private void confirmRestore(LocalBackup.Prepared prepared) {
        LocalBackup.Preview preview = prepared.preview();
        StringBuilder message = new StringBuilder("这份备份");
        String source = (preview.app() + " " + preview.appVersion()).trim();
        if (!source.isEmpty()) message.append("来自").append(source).append("，");
        String created = created(preview.createdAt());
        if (!created.isEmpty()) message.append("导出于 ").append(created).append("，");
        message.append("有 ").append(preview.phrases()).append(" 条常用语、").append(preview.words()).append(" 个词");
        // 旧版本导出的备份里没有输入记录，不提它。
        if (preview.learning() > 0) message.append("、").append(preview.learning()).append(" 条输入记录");
        message.append("和全部设置。")
            .append("\n\n恢复后设置按备份改写；皮肤、常用语、词和输入记录合并进来，本机已有的不会删除。");
        boolean[] decided = {false};
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("从备份恢复")
            .setMessage(message)
            .setNegativeButton("取消", null)
            .setPositiveButton("恢复", (dialog, which) -> {
                decided[0] = true;
                restore(prepared);
            })
            .setOnDismissListener(dialog -> {
                if (!decided[0]) discard(prepared);
            })
            .show();
    }

    private void restore(LocalBackup.Prepared prepared) {
        if (getView() == null) {
            discard(prepared);
            return;
        }
        setBusy(true);
        MsToast.show(requireContext(), "正在恢复…");
        HostTask.run(this, context -> LocalBackup.restore(context, prepared.archive()), message -> {
            setBusy(false);
            showResult("从备份恢复", message == null ? "没有恢复成功，请重试。" : message);
        });
    }

    /** 用户取消时删掉复制进缓存的那份备份；页面已经离开时交给系统清缓存。 */
    private void discard(LocalBackup.Prepared prepared) {
        if (getView() == null) return;
        HostTask.run(this, context -> {
            LocalBackup.discard(prepared.archive());
            return Boolean.TRUE;
        }, ignored -> {});
    }

    private void showResult(String title, String message) {
        if (getContext() == null) return;
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle(title)
            .setMessage(message)
            .setPositiveButton("好", null)
            .show();
    }

    /** manifest 里的 ISO 时间显示成本地的「2026-10-08 09:15」；读不出来时不显示。 */
    private static String created(String iso) {
        if (iso == null || iso.isEmpty()) return "";
        try {
            return DAY.format(Instant.parse(iso).atZone(ZoneId.systemDefault()));
        } catch (DateTimeParseException unreadable) {
            return "";
        }
    }
}
