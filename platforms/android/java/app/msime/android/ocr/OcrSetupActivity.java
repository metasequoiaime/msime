package app.msime.android;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
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
    private Button download;

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
        text(panel, "从键盘的「更多工具 → 拍照识字」打开独立窗口，拍照后裁剪或涂抹识别范围，点选文字复制或插入原输入框。模块不在基础安装包中，也不会自动下载。", 16);
        text(panel, "仅导入本项目提供的配套模块，安装器按宿主内固定的完整文件摘要校验。识别图片和文字在本机离线处理，不自动存图、复制或上屏；模块关闭了 ONNX Runtime 的遥测。请阅读模块附带的许可与隐私说明后再导入。", 14);
        status = text(panel, "正在读取模块状态…", 16);
        download = button(panel, "下载地址", this::openDownload);
        download.setEnabled(false);
        text(panel, "用浏览器或下载工具下载配套模块，再点下方按钮选择文件导入。", 14);
        choose = button(panel, "选择已下载的配套模块", this::choose);
        choose.setEnabled(false);
        remove = button(panel, "卸载配套模块", this::confirmRemove);
        remove.setEnabled(false);
        permission = button(panel, "允许使用相机", () -> requestPermissions(new String[]{Manifest.permission.CAMERA}, CAMERA_PERMISSION));
        text(panel, "相机只在已授权且拍照页位于前台时使用，离开拍照页就释放。照片和文字只在窗口内存中保留；原输入位置变化后不能插入。授权或导入完成后，请回到原输入框重新打开拍照识字。", 14);
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
                // 卸载状态放在进程里：页面重建后仍能看到进行中的卸载，不会在删除途中重新开放导入。
                boolean removing = ResourcePackService.removing(ResourcePacks.OCR_MODULE);
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
                download.setEnabled(supported && found != null && !found.importFiles().isEmpty()
                    && !found.importFiles().get(0).url().isEmpty());
            });
        });
    }

    /** 地址来自共享层固定的模块锁，交给用户选择的外部应用下载。 */
    private void openDownload() {
        if (AppVersionPolicy.playDistribution() || pack == null || pack.importFiles().isEmpty()) return;
        String url = pack.importFiles().get(0).url();
        if (url.isEmpty()) return;
        Intent view = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
        // 系统选择器在没有匹配应用时不会抛异常，先检查才能显示下载指引。
        if (getPackageManager().queryIntentActivities(view, 0).isEmpty()) {
            Toast.makeText(this, "没有可以打开下载地址的应用，请安装浏览器或下载工具后重试", Toast.LENGTH_LONG).show();
            return;
        }
        try {
            startActivity(Intent.createChooser(view, "选择浏览器或下载工具"));
        } catch (ActivityNotFoundException | SecurityException unavailable) {
            Toast.makeText(this, "没有可以打开下载地址的应用，请安装浏览器或下载工具后重试", Toast.LENGTH_LONG).show();
        }
    }

    private void choose() {
        if (pack == null) return;
        startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"), PICK_MODULE);
    }

    private void confirmRemove() {
        if (pack == null || ResourcePackService.removing(ResourcePacks.OCR_MODULE)) return;
        ScreenCaptureProtection.show(new AlertDialog.Builder(this)
            .setTitle("卸载配套模块？")
            .setMessage("卸载后将无法拍照识字，可重新导入恢复。你下载的原始文件和其他输入法数据会保留。")
            .setNegativeButton("取消", null)
            .setPositiveButton("卸载", (dialog, which) -> removeModule())
            .create());
    }

    /** 卸载交给进程级的资源包服务：与下载、导入在同一处互斥，页面重建或关闭不会丢掉进行中的卸载；结束时经监听器刷新本页。 */
    private void removeModule() {
        if (ResourcePackService.remove(this, ResourcePacks.OCR_MODULE)) refresh();
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PICK_MODULE || result != RESULT_OK || data == null || pack == null) return;
        if (ResourcePackService.removing(ResourcePacks.OCR_MODULE)) {
            Toast.makeText(this, "配套模块正在卸载，请等卸载完成后再导入", Toast.LENGTH_LONG).show();
            return;
        }
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
