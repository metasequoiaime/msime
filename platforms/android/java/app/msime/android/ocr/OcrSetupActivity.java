package app.msime.android;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Bundle;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import androidx.annotation.NonNull;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** 主进程承接可选模块导入和用户主动申请相机权限，输入法不启动安装器。 */
@SuppressWarnings("deprecation")
public final class OcrSetupActivity extends Activity implements ResourcePackService.Listener {
    private static final int PICK_MODULE = 9207;
    private static final int CAMERA_PERMISSION = 9208;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private ResourcePacks.Pack pack;
    private TextView status;
    private Button permission;
    private Button choose;
    private Button remove;
    private boolean removing;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        LinearLayout panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        int padding = Math.round(20 * getResources().getDisplayMetrics().density);
        panel.setPadding(padding, padding, padding, padding);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(panel);
        setContentView(scroll);
        text(panel, "离线拍照识字", 22);
        text(panel, "在键盘的「更多工具 → 拍照识字」里预览相机、拍照、选择文字上屏。模块不在基础安装包中，也不会自动下载。", 16);
        text(panel, "仅导入本项目提供的配套模块，安装器按宿主内固定的完整文件摘要校验。识别图片和文字在本机离线处理，不自动存图、复制或上屏；模块关闭了 ONNX Runtime 的遥测。请阅读模块附带的许可与隐私说明后再导入。", 14);
        status = text(panel, "正在读取模块状态…", 16);
        choose = button(panel, "选择已下载的配套模块", this::choose);
        choose.setEnabled(false);
        remove = button(panel, "卸载配套模块", this::confirmRemove);
        remove.setEnabled(false);
        permission = button(panel, "允许使用相机", () -> requestPermissions(new String[]{Manifest.permission.CAMERA}, CAMERA_PERMISSION));
        text(panel, "相机只在你打开拍照面板并已授权时使用。离开输入框或关闭面板就释放相机，待选文字同时清空。授权或导入完成后，请回到原输入框重新打开拍照识字。", 14);
        button(panel, "返回", this::finish);
    }

    @Override protected void onResume() {
        super.onResume();
        ResourcePackService.addListener(this);
        permission.setEnabled(checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED);
        permission.setText(permission.isEnabled() ? "允许使用相机" : "相机权限已允许（可在系统设置撤回）");
        refresh();
    }

    @Override protected void onPause() {
        ResourcePackService.removeListener(this);
        super.onPause();
    }

    @Override protected void onDestroy() {
        worker.shutdownNow();
        super.onDestroy();
    }

    @Override public void onChanged(@NonNull String id, boolean finished) { if (ResourcePacks.OCR_MODULE.equals(id)) refresh(); }

    private void refresh() {
        worker.execute(() -> {
            ResourcePacks.Pack found = ResourcePacks.pack(getFilesDir(), ResourcePacks.OCR_MODULE);
            runOnUiThread(() -> {
                if (isDestroyed()) return;
                pack = found;
                boolean supported = !AppVersionPolicy.playDistribution();
                ResourcePackService.Status progress = ResourcePackService.status(ResourcePacks.OCR_MODULE);
                String description = ResourcePackService.describe(progress);
                status.setText(removing ? "正在卸载配套模块…"
                    : !supported ? "此分发渠道不提供外部可执行模块。"
                    : description != null ? description
                    : found == null ? "配套模块尚未提供，当前版本暂不能导入。"
                    : found.installed() ? "配套模块已导入。"
                    : "尚未导入模块，请先从本项目下载与当前版本匹配的配套模块。");
                boolean busy = removing || (progress != null && progress.running());
                choose.setEnabled(supported && found != null && !busy);
                remove.setEnabled(found != null && !"missing".equals(found.state()) && !busy);
            });
        });
    }

    private void choose() {
        if (pack == null) return;
        startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"), PICK_MODULE);
    }

    private void confirmRemove() {
        if (pack == null || removing) return;
        ScreenCaptureProtection.show(new AlertDialog.Builder(this)
            .setTitle("卸载配套模块？")
            .setMessage("卸载后将无法拍照识字，可重新导入恢复。你下载的原始文件和其他输入法数据会保留。")
            .setNegativeButton("取消", null)
            .setPositiveButton("卸载", (dialog, which) -> removeModule())
            .create());
    }

    private void removeModule() {
        if (removing) return;
        removing = true;
        choose.setEnabled(false);
        remove.setEnabled(false);
        status.setText("正在卸载配套模块…");
        worker.execute(() -> {
            boolean success;
            try {
                ResourcePackService.Status progress = ResourcePackService.status(ResourcePacks.OCR_MODULE);
                if (progress != null && progress.running()) throw new ResourcePacks.Failure("resource_pack_busy");
                ResourcePacks.remove(getApplicationContext(), ResourcePacks.OCR_MODULE);
                success = true;
            } catch (ResourcePacks.Failure failure) {
                success = false;
            }
            boolean removed = success;
            runOnUiThread(() -> {
                if (removed) ResourcePackService.clearFinishedStatus(ResourcePacks.OCR_MODULE);
                if (isDestroyed()) return;
                removing = false;
                Toast.makeText(this, removed ? "配套模块已卸载" : "卸载失败，请稍后重试", Toast.LENGTH_LONG).show();
                refresh();
            });
        });
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_MODULE || result != RESULT_OK || data == null || pack == null) return;
        Uri uri = data.getData();
        if (uri == null) return;
        long[] sizes = pack.importFiles().stream().mapToLong(ResourcePacks.ImportFile::size).toArray();
        ResourcePackService.requestImport(this, ResourcePacks.OCR_MODULE, null, List.of(uri), sizes);
        refresh();
    }

    private TextView text(LinearLayout panel, String value, int size) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextSize(size);
        view.setPadding(0, 12, 0, 12);
        panel.addView(view);
        return view;
    }

    private Button button(LinearLayout panel, String title, Runnable action) {
        Button button = new Button(this);
        button.setText(title);
        button.setAllCaps(false);
        button.setOnClickListener(view -> action.run());
        panel.addView(button);
        return button;
    }
}
