package app.msime.android.home;

import app.msime.android.ClipboardExport;
import app.msime.android.ExportDirectory;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.widget.TextView;
import androidx.activity.result.ActivityResult;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import androidx.core.content.ContextCompat;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.ClipboardOperationAuthenticationPolicy;
import java.io.IOException;

/** 设置页与历史页共用目录选择和显式导出动作；结果由 HostTask 绑定当前视图。 */
final class ClipboardExportActions {
    private final ReloadingDetailPage page;
    private final Runnable reload;
    private final ActivityResultLauncher<Intent> picker;
    private boolean exportAfterPicking;
    private boolean busy;
    private TextView button;
    private BiometricPrompt authentication;

    ClipboardExportActions(ReloadingDetailPage page, Runnable reload) {
        this.page = page;
        this.reload = reload;
        picker = page.registerForActivityResult(new ActivityResultContracts.StartActivityForResult(), this::selected);
    }

    void restore(Bundle state) { exportAfterPicking = state != null && state.getBoolean("clipboardExportAfterPicking"); }
    void save(Bundle state) { state.putBoolean("clipboardExportAfterPicking", exportAfterPicking); }
    void destroyed() {
        if (authentication != null) authentication.cancelAuthentication();
        authentication = null;
        busy = false;
        button = null;
    }

    TextView button() {
        var context = page.requireContext();
        button = app.msime.android.ViewPolicy.textButton(context, "导出", 15, 500,
            app.msime.android.ThemeColorPolicy.accent(context), null, 48, this::export);
        app.msime.android.ViewPolicy.setPaddingDp(context, button, 12, 8, 12, 8);
        button.setMinHeight(app.msime.android.DimensionPolicy.pixels(context, 48));
        button.setContentDescription("导出全部剪贴板历史");
        button.setEnabled(!busy);
        return button;
    }

    void export() {
        if (busy) return;
        if (ClipboardOperationAuthenticationPolicy.requiresAuthentication()) {
            requestAuthentication();
            return;
        }
        exportAfterAuthentication();
    }

    private void requestAuthentication() {
        if (!ClipboardOperationAuthenticationPolicy.shouldStartAuthentication(authentication != null)) return;
        int authenticators = BiometricManager.Authenticators.BIOMETRIC_STRONG
            | BiometricManager.Authenticators.DEVICE_CREDENTIAL;
        android.content.Context context = page.requireContext();
        if (BiometricManager.from(context).canAuthenticate(authenticators)
                != BiometricManager.BIOMETRIC_SUCCESS) {
            MsToast.show(context, "这台设备没有可用的系统认证");
            return;
        }
        authentication = new BiometricPrompt(page, ContextCompat.getMainExecutor(context),
            new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult value) {
                    authentication = null;
                    if (page.getView() != null) exportAfterAuthentication();
                }

                @Override public void onAuthenticationError(int code, CharSequence message) {
                    authentication = null;
                }
            });
        BiometricPrompt.PromptInfo info = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle("导出剪贴板历史包含本机记录")
            .setAllowedAuthenticators(authenticators)
            .build();
        authentication.authenticate(info);
    }

    private void exportAfterAuthentication() {
        setBusy(true);
        HostTask.run(page, context -> ClipboardExport.export(context), message -> {
            setBusy(false);
            if (ClipboardExport.NEEDS_DIRECTORY.equals(message)) pick(true);
            else MsToast.show(page.requireContext(), message == null ? "剪贴板导出失败，请重试" : message);
        });
    }

    void choose() {
        if (busy) return;
        OptionSheet sheet = new OptionSheet(page.requireContext(), "剪贴板导出文件夹", null);
        sheet.option("Download（默认）", false, () -> {
            setBusy(true);
            HostTask.run(page, context -> {
                try {
                    String previous = ClipboardExport.saved(context);
                    AndroidLocalSettings.put(context, AndroidLocalSettings.CLIPBOARD_EXPORT_TREE, "");
                    if (!previous.isEmpty()) ExportDirectory.releaseIfUnused(context, previous);
                    return Boolean.TRUE;
                } catch (IOException | RuntimeException failed) { return null; }
            }, done -> {
                setBusy(false);
                reload.run();
                MsToast.show(page.requireContext(), done == null ? "设置失败，请重试" : "已恢复默认 Download 文件夹");
            });
        });
        sheet.option("选择其他文件夹", false, () -> pick(false));
        sheet.show();
    }

    private void pick(boolean thenExport) {
        exportAfterPicking = thenExport;
        setBusy(true);
        HostTask.run(page, ClipboardExport::saved, previous -> {
            setBusy(false);
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
            intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, previous == null || previous.isEmpty()
                ? Uri.parse("content://com.android.externalstorage.documents/document/primary%3ADownload") : Uri.parse(previous));
            try { picker.launch(intent); }
            catch (ActivityNotFoundException unavailable) {
                exportAfterPicking = false;
                MsToast.show(page.requireContext(), "系统文件夹选择器不可用");
            }
        });
    }

    private void selected(ActivityResult result) {
        boolean thenExport = exportAfterPicking;
        exportAfterPicking = false;
        Intent data = result.getData();
        if (result.getResultCode() != Activity.RESULT_OK || data == null || data.getData() == null
                || page.getView() == null || busy) return;
        Uri selected = data.getData();
        int flags = data.getFlags();
        setBusy(true);
        HostTask.run(page, context -> {
            try {
                ExportDirectory.remember(context, AndroidLocalSettings.CLIPBOARD_EXPORT_TREE, selected, flags);
                return thenExport ? ClipboardExport.export(context) : "已设置剪贴板导出文件夹";
            } catch (IOException | RuntimeException failed) {
                return "无法保存文件夹，请选择可读写的文件夹并允许长期访问";
            }
        }, message -> {
            setBusy(false);
            reload.run();
            MsToast.show(page.requireContext(), message == null ? "剪贴板导出失败，请重试" : message);
        });
    }

    private void setBusy(boolean value) {
        busy = value;
        if (button != null) button.setEnabled(!value);
    }
}
