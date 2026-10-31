package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.os.SystemClock;
import android.system.Os;
import android.system.OsConstants;
import android.view.accessibility.AccessibilityNodeInfo;
import app.msime.ocr.fixture.SyntheticOcrFixture;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.RandomAccessFile;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.function.Predicate;

/** 只操作专用模拟器中的合成模块、相机图和编辑器，不导出任意用户内容。 */
public final class OcrProductDeviceSmoke extends DeviceSmoke {
    private Class<?> runtime;
    private Method open;
    private Method install;
    private File source;
    private boolean permissionDenied;
    private boolean prepareHandwriting;
    private boolean handwritingCoexistence;
    private boolean pickerImport;
    private HostHandwriting handwriting;
    private List<?> handwritingBefore;

    @Override public void onCreate(android.os.Bundle arguments) {
        permissionDenied = arguments != null && "true".equals(arguments.getString("ocr-permission-denied"));
        prepareHandwriting = arguments != null && "true".equals(arguments.getString("ocr-handwriting-prepare"));
        handwritingCoexistence = arguments != null && "true".equals(arguments.getString("ocr-handwriting-coexistence"));
        pickerImport = arguments != null && "true".equals(arguments.getString("ocr-picker-import"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return prepareHandwriting ? "host handwriting model prepared for offline OCR coexistence"
            : pickerImport ? "system document picker imports verified OCR module and preserves it after cancellation and corrupt replacement"
            : handwritingCoexistence ? "same host handwriting recognizer infers before and after isolated OCR offline"
            : permissionDenied ? "revoked camera permission blocks native OCR preview"
            : "pinned OCR import, readonly isolated loading, camera capture and editor insertion";
    }

    @Override protected void runChecks() throws Exception {
        try {
            if (prepareHandwriting) {
                stage = "prepare host handwriting model";
                handwriting = new HostHandwriting(getTargetContext());
                handwriting.prepare();
            } else if (pickerImport) pickerChecks();
            else if (handwritingCoexistence) moduleChecks();
            else if (permissionDenied) permissionDeniedChecks();
            else productChecks();
        }
        catch (Exception | AssertionError error) {
            try { screenshot(prepareHandwriting ? "failure-handwriting-model"
                : pickerImport ? "failure-picker-import"
                : handwritingCoexistence ? "failure-handwriting-coexistence" : "failure"); }
            catch (Exception ignored) { }
            throw error;
        }
        finally { if (handwriting != null) handwriting.close(); }
    }

    /** 用真正的系统文档选择器跨 UID 传递 URI，不直接调用导入回调或安装接口。 */
    private void pickerChecks() throws Exception {
        check(getContext().getApplicationInfo().uid != getTargetContext().getApplicationInfo().uid,
            "document provider and product have different UIDs");
        android.accessibilityservice.AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        automation.setServiceInfo(info);
        stage = "clear synthetic module before picker";
        File previous = new File(getTargetContext().getFilesDir(), "bootstrap/state/resource-packs/ocr-module-android");
        if (previous.exists()) clearSyntheticModule(previous, "");
        runtime = getTargetContext().getClassLoader().loadClass("app.msime.android.OcrRuntime");
        open = runtime.getMethod("open", Context.class);
        android.app.Activity activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.OcrSetupActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        try {
            stage = "cancel first system picker";
            openPicker();
            check(automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK), "cancel system document picker");
            awaitAny(node -> equalsText("选择已下载的配套模块", node.getText()) && node.isEnabled());
            rejectOpen("cancelled initial import");

            stage = "reject corrupt picker import";
            pickModule("corrupt");
            importFailed();
            rejectOpen("corrupt initial picker import");
            screenshot("picker-rejected");

            stage = "import verified module through system picker";
            pickModule("valid");
            awaitAny(node -> equalsText("配套模块已导入。", node.getText()));
            recognizeFixture();
            screenshot("picker-imported");

            stage = "cancel replacement system picker";
            openPicker();
            check(automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK), "cancel replacement picker");
            awaitAny(node -> equalsText("选择已下载的配套模块", node.getText()) && node.isEnabled());
            recognizeFixture();

            stage = "reject corrupt picker replacement";
            pickModule("corrupt");
            importFailed();
            recognizeFixture();
            screenshot("picker-preserved");

            stage = "cancel module uninstall";
            tap(node -> equalsText("卸载配套模块", node.getText()) && node.isEnabled());
            awaitAny(node -> equalsText("卸载配套模块？", node.getText()));
            tap(node -> equalsText("取消", node.getText()) && node.isClickable());
            recognizeFixture();

            stage = "uninstall imported module after failed replacement";
            tap(node -> equalsText("卸载配套模块", node.getText()) && node.isEnabled());
            awaitAny(node -> equalsText("卸载配套模块？", node.getText()));
            tap(node -> equalsText("卸载", node.getText()) && node.isClickable());
            awaitAny(node -> node.getText() != null && node.getText().toString().startsWith("尚未导入模块"));
            awaitAny(node -> equalsText("卸载配套模块", node.getText()) && !node.isEnabled());
            rejectOpen("uninstalled module cannot reopen through the cached loader");
            screenshot("picker-uninstalled");

            stage = "reimport module after uninstall";
            pickModule("valid");
            awaitAny(node -> equalsText("配套模块已导入。", node.getText()));
            recognizeFixture();
            screenshot("picker-reimported");
        } finally { runOnMainSync(activity::finish); }
    }

    private void importFailed() throws Exception {
        awaitAny(node -> node.getText() != null && node.getText().toString().startsWith("导入失败："));
        awaitAny(node -> equalsText("选择已下载的配套模块", node.getText()) && node.isEnabled());
    }

    private void openPicker() throws Exception {
        tap(node -> equalsText("选择已下载的配套模块", node.getText()) && node.isEnabled());
        awaitAny(this::pickerNode);
    }

    private void pickModule(String id) throws Exception {
        String operation = stage;
        stage = operation + " / open picker";
        openPicker();
        stage = operation + " / show document roots";
        clickPicker(node -> pickerNode(node) && node.getContentDescription() != null
            && (node.getContentDescription().toString().equalsIgnoreCase("Show roots")
                || node.getContentDescription().toString().contains("根目录")
                || node.getContentDescription().toString().contains("导航")));
        stage = operation + " / choose synthetic provider";
        clickPicker(node -> pickerNode(node) && equalsText("MSIME OCR 测试模块", node.getText())
            && "android:id/title".equals(node.getViewIdResourceName()));
        stage = operation + " / choose synthetic document";
        Predicate<AccessibilityNodeInfo> document = node -> pickerNode(node)
            && equalsText("msime-synthetic-ocr-" + id + ".apk", node.getText()) && node.isEnabled();
        AccessibilityNodeInfo target = awaitStableBounds(document);
        android.graphics.Rect bounds = new android.graphics.Rect();
        target.getBoundsInScreen(bounds);
        check(bounds.width() > 0 && bounds.height() > 0, "synthetic document has visible touch bounds");
        screenshot("picker-documents");
        // Android 9 的文档条目不提供可点击节点；只对这个已确认位置的合成文件实走触摸。
        if (android.os.Build.VERSION.SDK_INT == 28) tap(document);
        else clickPicker(document);
        stage = operation;
    }

    /** 点击系统文档控件或同包可点击父节点，避免抽屉动画使坐标落到别的根。 */
    private void clickPicker(Predicate<AccessibilityNodeInfo> match) throws Exception {
        for (int attempt = 0; attempt < 3; attempt++) {
            dropStaleNodes();
            automation.waitForIdle(500, 5000);
            AccessibilityNodeInfo target = awaitAny(match);
            if (!target.isVisibleToUser()) {
                check(target.performAction(
                    AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()),
                    "show synthetic document picker control");
                continue;
            }
            while (target != null && pickerNode(target) && !target.isClickable()) target = target.getParent();
            check(target != null && pickerNode(target) && target.isClickable(), "clickable document picker control");
            if (target.performAction(AccessibilityNodeInfo.ACTION_CLICK)) return;
        }
        throw new AssertionError("System document picker action failed");
    }

    private boolean pickerNode(AccessibilityNodeInfo node) {
        return node.getPackageName() != null && node.getPackageName().toString().endsWith(".documentsui");
    }

    /** 撤权会终止目标进程，由脚本在两次 instrumentation 之间撤回权限。 */
    private void permissionDeniedChecks() throws Exception {
        stage = "camera permission revoked";
        check(getTargetContext().checkSelfPermission("android.permission.CAMERA")
            != android.content.pm.PackageManager.PERMISSION_GRANTED, "camera permission is revoked");
        restartIme();
        editor();
        tap(key("更多"));
        showTool("拍照识字");
        tap(tool("拍照识字"));
        awaitAny(node -> equalsText("请先在拍照识字设置中授权相机并导入配套模块。", node.getText()));
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            check(findAny(window.getRoot(), described("拍照识字相机预览")) == null,
                "revoked permission cannot create camera preview");
        screenshot("permission");
    }

    private void moduleChecks() throws Exception {
        Context context = getTargetContext();
        stage = "offline prerequisite";
        long offlineDeadline = SystemClock.uptimeMillis() + 10000;
        android.net.ConnectivityManager connectivity = context.getSystemService(android.net.ConnectivityManager.class);
        while (connectivity.getActiveNetwork() != null && SystemClock.uptimeMillis() < offlineDeadline) SystemClock.sleep(100);
        check(connectivity.getActiveNetwork() == null, "product OCR test runs without an active network");
        stage = "clear prior synthetic module";
        File previous = new File(context.getFilesDir(), "bootstrap/state/resource-packs/ocr-module-android");
        if (previous.exists()) clearSyntheticModule(previous, "");
        ClassLoader loader = context.getClassLoader();
        runtime = loader.loadClass("app.msime.android.OcrRuntime");
        open = runtime.getMethod("open", Context.class);
        Class<?> progress = loader.loadClass("app.msime.android.NativeClient$ResourcePackProgress");
        install = loader.loadClass("app.msime.android.ResourcePacks").getMethod("install",
            Context.class, String.class, List.class, List.class, progress);
        stage = "host handwriting client before OCR";
        handwritingClient();
        // 本地语音识别的 sherpa-onnx 自带另一份 libonnxruntime.so，加载在宿主自己的链接器命名空间；OCR 模块的 ONNX Runtime 在模块类加载器的命名空间里。两份先后进同一个进程后，OCR 和语音运行库都必须照常可用。
        stage = "host speech runtime before OCR";
        Method speech = loader.loadClass("app.msime.android.NativeClient").getMethod("localSpeechAvailable");
        boolean speechRuntime = (Boolean) speech.invoke(null);
        android.util.Log.i("MSIME_OCR_PRODUCT", "speech_runtime_loaded_before_ocr=" + speechRuntime);
        source = new File(context.getCacheDir(), "synthetic-ocr-import.apk");
        try (InputStream input = getContext().getAssets().open("ocr-product-module.apk");
             FileOutputStream output = new FileOutputStream(source)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
        stage = "reject missing and corrupt module";
        rejectOpen("missing module");
        corrupt(source);
        rejectImport("corrupt module");
        copySource();
        stage = "import and recognize readonly module";
        File directory = importModule();
        File apk = new File(directory, "msime-ocr-v1.apk");
        check((Os.stat(apk.getAbsolutePath()).st_mode & 0222) == 0, "import seals executable readonly");
        recognizeFixture();
        recognizeFixture();
        // 同长度错误导入不能替换已经能用的模块。
        stage = "reject replacements and tampering";
        corrupt(source);
        rejectImport("bad replacement");
        recognizeFixture();
        check(apk.setWritable(true, true), "test opens synthetic module permissions");
        rejectOpen("writable executable");
        check(apk.setReadOnly(), "test reseals synthetic module");
        corrupt(apk);
        rejectOpen("tampered published bytes");
        copySource();
        importModule();
        recognizeFixture();
        source.delete();
        stage = "host speech runtime after OCR";
        if (speechRuntime) check((Boolean) speech.invoke(null), "host speech runtime stays loaded beside the OCR runtime");
        stage = "host handwriting client after OCR";
        handwritingClient();
    }

    /** ART 编译产物所在的指令集目录名：arm64-v8a 设备是 `arm64`，x86_64 设备是 `x86_64`。 */
    private static String artInstructionSet() {
        String abi = android.os.Build.SUPPORTED_64_BIT_ABIS.length == 0 ? "" : android.os.Build.SUPPORTED_64_BIT_ABIS[0];
        return abi.equals("arm64-v8a") ? "arm64" : abi;
    }

    /** ART 还会生成性能采样和锁；只清理固定合成模块的已知条目，不跟随符号链接。 */
    private void clearSyntheticModule(File path, String relative) throws Exception {
        int mode = Os.lstat(path.getAbsolutePath()).st_mode;
        String isa = artInstructionSet();
        if (relative.isEmpty() || relative.equals("oat") || relative.equals("oat/" + isa)) {
            check(OsConstants.S_ISDIR(mode), "synthetic module cache is a real directory");
            File[] entries = path.listFiles();
            check(entries != null, "synthetic module directory readable");
            for (File entry : entries)
                clearSyntheticModule(entry, relative.isEmpty() ? entry.getName() : relative + "/" + entry.getName());
        } else {
            check(relative.equals("msime-ocr-v1.apk") || relative.equals("msime-model.json")
                || relative.equals("msime-ocr-v1.apk." + isa + ".flock")
                || relative.equals("oat/msime-ocr-v1.apk.cur.prof")
                || relative.equals("oat/msime-ocr-v1.apk.prof")
                || relative.equals("oat/" + isa + "/msime-ocr-v1.odex")
                || relative.equals("oat/" + isa + "/msime-ocr-v1.vdex")
                || relative.equals("oat/" + isa + "/msime-ocr-v1.art"), "only known synthetic module files can be removed: "
                    + (relative.matches("(?:oat/|oat/(?:x86_64|arm64)/)?msime-ocr-v1[.a-z0-9_-]{0,80}")
                        ? relative : "unrecognized path"));
            check(OsConstants.S_ISREG(mode), "synthetic module cache is a regular file");
        }
        check(path.delete(), "remove prior synthetic module entry");
    }

    private void productChecks() throws Exception {
        moduleChecks();
        photoSelectionChecks();
        Context context = getTargetContext();
        stage = "native independent camera preview";
        automation.grantRuntimePermission(context.getPackageName(), "android.permission.CAMERA");
        captureDiagnostic();
        restartIme();
        editor();
        AccessibilityNodeInfo initial = awaitAny(field("msime-test-plain"));
        android.os.Bundle prefix = new android.os.Bundle();
        prefix.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "OCR前文");
        check(initial.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, prefix), "seed synthetic editor context");
        awaitAny(field("msime-test-plain").and(node -> equalsText("OCR前文", node.getText())
            && node.getTextSelectionStart() == 5 && node.getTextSelectionEnd() == 5));
        camera();
        screenshot("preview");
        stage = "capture, process photo and recognize through native IME";
        captureAndRecognize();
        screenshot("result");
        stage = "insert all OCR text into the original editor";
        tap(node -> equalsText("插入", node.getText()) && node.isEnabled());
        dropStaleNodes();
        awaitAny(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
            && node.getText().toString().startsWith("OCR前文")
            && node.getText().toString().replaceAll("\\s", "").contains("SYNTHETICOCR2468")));
        screenshot("editor");

        stage = "select individual OCR characters without long press";
        editor();
        camera();
        captureAndRecognize();
        tap(described("全选文字"));
        for (char digit : "2468".toCharArray()) clickCharacter(digit);
        screenshot("selection");
        tap(node -> equalsText("复制", node.getText()));
        android.content.ClipboardManager clipboard = context.getSystemService(android.content.ClipboardManager.class);
        long copyDeadline = SystemClock.uptimeMillis() + 5000;
        while (SystemClock.uptimeMillis() < copyDeadline) {
            android.content.ClipData clip = clipboard.getPrimaryClip();
            if (clip != null && clip.getItemCount() == 1 && equalsText("2468", clip.getItemAt(0).getText())) break;
            SystemClock.sleep(100);
        }
        android.content.ClipData copied = clipboard.getPrimaryClip();
        check(copied != null && copied.getItemCount() == 1 && equalsText("2468", copied.getItemAt(0).getText()),
            "copy contains only individually selected digits");
        tap(node -> equalsText("插入", node.getText()) && node.isEnabled());
        awaitAny(field("msime-test-plain").and(node -> equalsText("2468", node.getText())));

        stage = "return from photo processing without inserting";
        camera();
        tap(node -> equalsText("拍照", node.getText()) && node.isEnabled());
        awaitAny(node -> equalsText("下一步", node.getText()));
        tap(node -> equalsText("返回键盘", node.getText()));
        awaitAny(key("更多"));
        awaitAny(field("msime-test-plain").and(node -> equalsText("2468", node.getText())));

        // 后台释放驱动，重新从键盘进入时应能再次取得相机。
        stage = "release preview when window backgrounds";
        camera();
        home("hidden");
        editor();
        camera();
        screenshot("resumed");
        captureAndRecognize();
        android.app.ActivityManager manager = context.getSystemService(android.app.ActivityManager.class);
        android.app.ActivityManager.AppTask ocrTask = null;
        for (android.app.ActivityManager.AppTask task : manager.getAppTasks()) {
            android.app.ActivityManager.RecentTaskInfo info = task.getTaskInfo();
            if (info.topActivity != null && info.topActivity.getClassName().equals("app.msime.android.OcrCaptureActivity"))
                ocrTask = task;
        }
        check(ocrTask != null, "independent OCR has its own native activity task");
        stage = "old result cannot insert into a different editor session";
        home("switched");
        editor();
        ocrTask.moveToFront();
        awaitAny(node -> equalsText("插入", node.getText()) && !node.isEnabled());
        tap(node -> equalsText("返回键盘", node.getText()));
        // 测试主动把旧任务置前，系统可能回到桌面；重新前置合成编辑器核对内容。
        editor();
        AccessibilityNodeInfo target = awaitAny(field("msime-test-plain"));
        check(target.getText() == null || target.getText().length() == 0 || target.isShowingHintText(),
            "old OCR result did not write into a new input target");

        stage = "photo survives orientation changes without saving to disk";
        camera();
        tap(node -> equalsText("拍照", node.getText()) && node.isEnabled());
        awaitAny(node -> equalsText("下一步", node.getText()));
        try {
            check(automation.setRotation(android.app.UiAutomation.ROTATION_FREEZE_90), "rotate synthetic OCR window");
            SystemClock.sleep(500);
            tap(node -> equalsText("下一步", node.getText()));
            awaitAny(node -> described("点选识别文字").test(node) && node.getText() != null
                && node.getText().toString().replaceAll("\\s", "").contains("SYNTHETICOCR2468"));
            screenshot("rotated");
        } finally {
            automation.setRotation(android.app.UiAutomation.ROTATION_FREEZE_0);
        }
        tap(node -> equalsText("重新拍照", node.getText()));
        awaitAny(node -> equalsText("拍照", node.getText()) && node.isEnabled());
        tap(node -> equalsText("返回键盘", node.getText()));
    }

    private void captureAndRecognize() throws Exception {
        tap(node -> equalsText("拍照", node.getText()) && node.isEnabled());
        awaitAny(node -> equalsText("下一步", node.getText()));
        screenshot("photo");
        tap(node -> equalsText("裁剪", node.getText()));
        awaitAny(described("拖动裁剪框四边或四角，选择识别区域"));
        tap(node -> equalsText("裁剪", node.getText()));
        tap(node -> equalsText("涂抹", node.getText()));
        tap(node -> equalsText("下一步", node.getText()));
        AccessibilityNodeInfo photo = awaitAny(described("涂抹要识别的区域，高亮部分将被识别"));
        android.graphics.Rect bounds = new android.graphics.Rect();
        photo.getBoundsInScreen(bounds);
        injectPhotoStroke(bounds.centerX() - 30, bounds.centerY(), bounds.centerX() + 30, bounds.centerY());
        tap(node -> equalsText("涂抹", node.getText()));
        tap(node -> equalsText("下一步", node.getText()));
        awaitAny(node -> equalsText("插入", node.getText()));
    }

    private void injectPhotoStroke(float fromX, float fromY, float toX, float toY) {
        long time = SystemClock.uptimeMillis();
        int[] actions = {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_MOVE, android.view.MotionEvent.ACTION_UP};
        for (int i = 0; i < actions.length; i++) {
            android.view.MotionEvent event = android.view.MotionEvent.obtain(time, time + i * 16, actions[i],
                i == 0 ? fromX : toX, i == 0 ? fromY : toY, 0);
            event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
            try { check(automation.injectInputEvent(event, true), "brush gesture reaches photo window"); }
            finally { event.recycle(); }
        }
    }

    /** 通过系统无障碍暴露的 ClickableSpan 点击单字，与用户短按走同一个选择回调。 */
    private void clickCharacter(char value) throws Exception {
        dropStaleNodes();
        AccessibilityNodeInfo node = awaitAny(described("点选识别文字"));
        CharSequence text = node.getText();
        check(text instanceof android.text.Spanned, "recognized text exposes accessible character actions");
        android.text.Spanned spans = (android.text.Spanned) text;
        int at = text.toString().indexOf(value);
        check(at >= 0, "synthetic digit exists in recognition result");
        android.text.style.ClickableSpan[] actions = spans.getSpans(at, at + 1, android.text.style.ClickableSpan.class);
        check(actions.length == 1, "one short-press action covers one synthetic digit");
        actions[0].onClick(null);
        SystemClock.sleep(150);
    }

    /** 缩放后的手势必须改变实际推理图：裁剪缩小输入，笔刷以白色屏蔽未画区域。 */
    private void photoSelectionChecks() throws Exception {
        stage = "photo selection pixel contract";
        Class<?> activity = getTargetContext().getClassLoader().loadClass("app.msime.android.OcrCaptureActivity");
        Method boundaries = activity.getDeclaredMethod("characterBoundaries", String.class);
        boundaries.setAccessible(true);
        int[] offsets = (int[]) boundaries.invoke(null, "中e\u0301👩‍💻🇨🇳");
        check(java.util.Arrays.equals(offsets, new int[]{0, 1, 3, 8, 12}),
            "character selection preserves combining marks, ZWJ emoji and flag pairs");
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.OcrPhotoSelectionView");
        var constructor = type.getDeclaredConstructor(Context.class, Bitmap.class);
        constructor.setAccessible(true);
        Class<?> modeType = getTargetContext().getClassLoader().loadClass("app.msime.android.OcrPhotoSelectionView$Mode");
        Method mode = type.getDeclaredMethod("mode", modeType);
        Method selection = type.getDeclaredMethod("selection");
        mode.setAccessible(true); selection.setAccessible(true);
        Bitmap photo = Bitmap.createBitmap(400, 200, Bitmap.Config.ARGB_8888);
        photo.eraseColor(android.graphics.Color.BLACK);
        final Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            try {
                android.view.View view = (android.view.View) constructor.newInstance(getTargetContext(), photo);
                android.widget.FrameLayout parent = new android.widget.FrameLayout(getTargetContext());
                parent.addView(view);
                view.layout(0, 0, 800, 600);
                Object[] modes = modeType.getEnumConstants();
                mode.invoke(view, modes[1]);
                // 原图以 2 倍缩放居中，四边从全图向内移动。
                selectionGesture(view, 0, 300, 100, 300);
                selectionGesture(view, 799, 300, 700, 300);
                selectionGesture(view, 400, 100, 400, 160);
                selectionGesture(view, 400, 499, 400, 440);
                Object area = selection.invoke(view);
                Method image = area.getClass().getDeclaredMethod("image");
                image.setAccessible(true);
                Bitmap cropped = (Bitmap) image.invoke(area);
                check(cropped.getWidth() == 300 && cropped.getHeight() == 140,
                    "four crop handles project from view space into image pixels");
                mode.invoke(view, modes[2]);
                check(selection.invoke(view) == null, "empty brush cannot start recognition");
                selectionGesture(view, 400, 300, 500, 300);
                Bitmap masked = (Bitmap) image.invoke(selection.invoke(view));
                check(masked.getPixel(200, 100) == android.graphics.Color.BLACK, "painted text pixels are preserved");
                check(masked.getPixel(0, 0) == android.graphics.Color.WHITE, "unpainted pixels are removed before OCR");
                check(masked.getWidth() == 400 && masked.getHeight() == 200, "brush keeps the original image coordinate frame");
                cropped.recycle(); masked.recycle();
            } catch (Throwable error) { failure[0] = error; }
        });
        photo.recycle();
        if (failure[0] != null) throw new AssertionError("photo selection contract", failure[0]);
    }

    private static void selectionGesture(android.view.View view, float fromX, float fromY, float toX, float toY) {
        long time = SystemClock.uptimeMillis();
        int[] actions = {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_MOVE, android.view.MotionEvent.ACTION_UP};
        for (int i = 0; i < actions.length; i++) {
            android.view.MotionEvent event = android.view.MotionEvent.obtain(time, time + i * 16, actions[i],
                i == 0 ? fromX : toX, i == 0 ? fromY : toY, 0);
            try { check(view.dispatchTouchEvent(event), "photo gesture is consumed"); }
            finally { event.recycle(); }
        }
    }

    /** HOME 请求异步切换任务，确认桌面已获得焦点且键盘消失后才能重开编辑器。 */
    private void home(String checkpoint) throws Exception {
        String launcher;
        // 通过测试的 shell 身份解析桌面，避免依赖产品进程的包可见性权限。
        try (InputStream input = new android.os.ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand("cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME"))) {
            java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[256];
            int count;
            while ((count = input.read(buffer)) != -1) bytes.write(buffer, 0, count);
            String output = new String(bytes.toByteArray(), java.nio.charset.StandardCharsets.UTF_8).trim();
            String component = output.substring(output.lastIndexOf('\n') + 1).trim();
            check(component.indexOf('/') > 0, "dedicated AVD has a resolved launcher");
            launcher = component.substring(0, component.indexOf('/'));
        }
        check(automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME), "request home transition");
        long deadline = SystemClock.uptimeMillis() + 15000;
        long activeSince = 0;
        do {
            dropStaleNodes();
            boolean activeHome = false, keyboard = false;
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo root = window.getRoot();
                if (window.getType() == android.view.accessibility.AccessibilityWindowInfo.TYPE_INPUT_METHOD) keyboard = true;
                if (window.isActive() && root != null && equalsText(launcher, root.getPackageName())) activeHome = true;
            }
            // 无障碍窗口先于退场动画更新，等待稳定后再保存画面并重开编辑器。
            if (activeHome && !keyboard) {
                if (activeSince == 0) activeSince = SystemClock.uptimeMillis();
                if (SystemClock.uptimeMillis() - activeSince >= 500) { screenshot(checkpoint); return; }
            } else activeSince = 0;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("home transition must finish before reopening synthetic editor");
    }

    private void copySource() throws Exception {
        try (InputStream input = getContext().getAssets().open("ocr-product-module.apk");
             FileOutputStream output = new FileOutputStream(source)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }

    /** 仅导出专用 AVD 的固定相机输入，经与产品相同的 JPEG 解码和 EXIF 路径诊断裁剪。 */
    @SuppressWarnings("deprecation")
    private void captureDiagnostic() throws Exception {
        stage = "synthetic camera JPEG diagnostic";
        android.app.Activity activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.OcrSetupActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        java.util.concurrent.CountDownLatch surfaceReady = new java.util.concurrent.CountDownLatch(1);
        android.view.SurfaceView[] surface = new android.view.SurfaceView[1];
        runOnMainSync(() -> {
            surface[0] = new android.view.SurfaceView(activity);
            surface[0].getHolder().addCallback(new android.view.SurfaceHolder.Callback() {
                @Override public void surfaceCreated(android.view.SurfaceHolder holder) { surfaceReady.countDown(); }
                @Override public void surfaceChanged(android.view.SurfaceHolder holder, int format, int width, int height) { }
                @Override public void surfaceDestroyed(android.view.SurfaceHolder holder) { }
            });
            activity.setContentView(surface[0]);
        });
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.OcrCamera");
        var constructor = type.getDeclaredConstructor();
        constructor.setAccessible(true);
        Object camera = constructor.newInstance();
        Method close = type.getDeclaredMethod("close");
        close.setAccessible(true);
        try {
            check(surfaceReady.await(15, java.util.concurrent.TimeUnit.SECONDS), "diagnostic surface ready");
            java.util.concurrent.CountDownLatch preview = new java.util.concurrent.CountDownLatch(1);
            java.util.concurrent.atomic.AtomicBoolean failed = new java.util.concurrent.atomic.AtomicBoolean();
            Method start = type.getDeclaredMethod("start", android.view.SurfaceHolder.class, int.class,
                java.util.function.BiConsumer.class, Runnable.class);
            start.setAccessible(true);
            int rotation = activity.getWindowManager().getDefaultDisplay().getRotation();
            start.invoke(camera, surface[0].getHolder(), rotation,
                (java.util.function.BiConsumer<Integer, Integer>) (width, height) -> preview.countDown(),
                (Runnable) () -> { failed.set(true); preview.countDown(); });
            check(preview.await(15, java.util.concurrent.TimeUnit.SECONDS) && !failed.get(), "diagnostic preview ready");
            SystemClock.sleep(500);
            java.util.concurrent.CountDownLatch captured = new java.util.concurrent.CountDownLatch(1);
            java.util.concurrent.atomic.AtomicReference<byte[]> bytes = new java.util.concurrent.atomic.AtomicReference<>();
            Method capture = type.getDeclaredMethod("capture", java.util.function.Consumer.class, Runnable.class);
            capture.setAccessible(true);
            capture.invoke(camera, (java.util.function.Consumer<byte[]>) value -> { bytes.set(value); captured.countDown(); },
                (Runnable) () -> { failed.set(true); captured.countDown(); });
            check(captured.await(15, java.util.concurrent.TimeUnit.SECONDS) && !failed.get(), "diagnostic JPEG captured");
            Method decode = loader.loadClass("app.msime.android.ImeOcr").getDeclaredMethod("decode", byte[].class);
            decode.setAccessible(true);
            Bitmap image = (Bitmap) decode.invoke(null, (Object) bytes.get());
            try (FileOutputStream output = new FileOutputStream(new File(
                    getTargetContext().getExternalFilesDir(null), "ocr-product-captured.png"))) {
                check(image.compress(Bitmap.CompressFormat.PNG, 100, output), "write synthetic camera diagnostic");
            } finally { image.recycle(); }
        } finally {
            close.invoke(camera);
            runOnMainSync(activity::finish);
        }
    }

    /** 共存专项保留同一识别器做前后真实推理，普通相机专项仍只检查客户端创建。 */
    private void handwritingClient() throws Exception {
        if (handwritingCoexistence) {
            if (handwriting == null) handwriting = new HostHandwriting(getTargetContext());
            List<?> recognized = handwriting.recognize();
            check(recognized.contains("中"), "host model recognizes the complete synthetic character");
            if (handwritingBefore == null) handwritingBefore = recognized;
            else check(handwritingBefore.equals(recognized), "same host recognizer keeps its inference across OCR");
            return;
        }
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.MlKitHandwritingRecognizer");
        Object client = type.getConstructor(Context.class).newInstance(getTargetContext());
        type.getMethod("close").invoke(client);
    }

    /** 只跨产品公开接口反射，不把 Google 类型或第二份 SDK 编进测试 APK。 */
    private static final class HostHandwriting {
        private final ClassLoader loader;
        private final Class<?> type;
        private final Object client;

        HostHandwriting(Context context) throws Exception {
            loader = context.getClassLoader();
            type = loader.loadClass("app.msime.android.MlKitHandwritingRecognizer");
            client = type.getConstructor(Context.class).newInstance(context);
        }

        void prepare() throws Exception {
            Class<?> listener = loader.loadClass("app.msime.android.HandwritingRecognizer$DownloadListener");
            var done = new java.util.concurrent.CountDownLatch(1);
            var success = new java.util.concurrent.atomic.AtomicBoolean();
            Object callback = Proxy.newProxyInstance(loader, new Class<?>[]{listener}, (proxy, method, values) -> {
                if (method.getName().equals("onComplete")) { success.set(true); done.countDown(); }
                if (method.getName().equals("onFailure")) done.countDown();
                return null;
            });
            type.getMethod("download", listener).invoke(client, callback);
            check(done.await(180, java.util.concurrent.TimeUnit.SECONDS) && success.get(),
                "host handwriting model download completes");
        }

        List<?> recognize() throws Exception {
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (!type.getMethod("availability").invoke(client).toString().equals("READY")) {
                check(SystemClock.uptimeMillis() < deadline, "host handwriting model ready while offline");
                SystemClock.sleep(100);
            }
            Class<?> point = loader.loadClass("app.msime.android.HandwritingInk$Point");
            var pointConstructor = point.getConstructor(float.class, float.class, long.class);
            // 与手写界面夹具相同的五笔「中」字，时间与坐标均为固定合成值。
            float[][] segments = {{88,100,88,272},{88,100,312,100},{312,100,312,272},
                {88,272,312,272},{200,40,200,360}};
            List<List<Object>> strokes = new ArrayList<>();
            long time = 1000;
            for (float[] segment : segments) {
                List<Object> stroke = new ArrayList<>();
                for (int step = 0; step <= 12; step++) {
                    float fraction = step / 12f;
                    stroke.add(pointConstructor.newInstance(segment[0] + (segment[2] - segment[0]) * fraction,
                        segment[1] + (segment[3] - segment[1]) * fraction, time));
                    time += 16;
                }
                strokes.add(stroke);
                time += 100;
            }
            Class<?> request = loader.loadClass("app.msime.android.HandwritingRecognizer$Request");
            Object ink = request.getConstructor(long.class, List.class, float.class, float.class)
                .newInstance(7L, strokes, 400f, 400f);
            Class<?> listener = loader.loadClass("app.msime.android.HandwritingRecognizer$RecognitionListener");
            var done = new java.util.concurrent.CountDownLatch(1);
            var result = new java.util.concurrent.atomic.AtomicReference<List<?>>();
            Object callback = Proxy.newProxyInstance(loader, new Class<?>[]{listener}, (proxy, method, values) -> {
                if (method.getName().equals("onResult") && Long.valueOf(7).equals(values[0])
                        && values[1] instanceof List<?> candidates) result.set(List.copyOf(candidates));
                if (method.getName().equals("onResult") || method.getName().equals("onFailure")) done.countDown();
                return null;
            });
            type.getMethod("recognize", request, listener).invoke(client, ink, callback);
            check(done.await(60, java.util.concurrent.TimeUnit.SECONDS) && result.get() != null,
                "host handwriting model returns a real inference");
            return result.get();
        }

        void close() throws Exception { type.getMethod("close").invoke(client); }
    }

    private File importModule() throws Exception {
        return (File) install.invoke(null, getTargetContext(), "ocr-module-android",
            Collections.emptyList(), Collections.singletonList(source.getAbsolutePath()), null);
    }

    private void rejectImport(String purpose) throws Exception {
        try { importModule(); }
        catch (InvocationTargetException rejected) { return; }
        throw new AssertionError("Import accepted " + purpose);
    }

    private void rejectOpen(String purpose) throws Exception {
        try {
            Object module = open.invoke(null, getTargetContext());
            runtime.getMethod("close").invoke(module);
        } catch (InvocationTargetException rejected) { return; }
        throw new AssertionError("Runtime accepted " + purpose);
    }

    private void recognizeFixture() throws Exception {
        Object module;
        try { module = open.invoke(null, getTargetContext()); }
        catch (InvocationTargetException error) {
            Throwable cause = error;
            while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
            // 仅输出异常类型和源码位置，不输出模块异常消息或识别内容。
            String location = cause.getStackTrace().length == 0 ? "" : " at " + cause.getStackTrace()[0];
            throw new AssertionError("runtime open " + cause.getClass().getName() + location);
        }
        Bitmap image = SyntheticOcrFixture.render(1);
        try {
            String[] lines = (String[]) runtime.getMethod("recognize", Bitmap.class).invoke(module, image);
            check(SyntheticOcrFixture.matches(lines), "isolated product runtime recognizes complete Chinese and Latin fixture");
        } finally {
            runtime.getMethod("close").invoke(module);
            image.recycle();
        }
    }

    private static void corrupt(File file) throws Exception {
        boolean sealed = (Os.stat(file.getAbsolutePath()).st_mode & 0222) == 0;
        if (sealed) check(file.setWritable(true, true), "test unseals synthetic bytes");
        try (RandomAccessFile output = new RandomAccessFile(file, "rw")) {
            int value = output.read();
            output.seek(0);
            output.write(value ^ 0xff);
        }
        if (sealed) check(file.setReadOnly(), "test reseals corrupt bytes");
    }

    private void editor() throws Exception {
        // 从桌面重开合成编辑器，使用测试 shell 等待启动完成，不依赖目标进程的后台启动权限。
        shellDiscard("am start -W -n app.msime.android.test/.EditorActivity -f 0x10008000");
        dropStaleNodes();
        tap(field("msime-test-plain"));
        awaitAny(key("更多"));
    }

    private void camera() throws Exception {
        stage = "open independent camera from actual keyboard toolbar";
        dropStaleNodes();
        tap(key("更多"));
        showTool("拍照识字");
        tap(tool("拍照识字"));
        AccessibilityNodeInfo capture = awaitAny(node -> equalsText("拍照", node.getText()) && node.isEnabled());
        android.graphics.Rect bounds = new android.graphics.Rect();
        capture.getBoundsInScreen(bounds);
        check(bounds.width() > 0 && bounds.height() > 0, "capture control has visible touch bounds");
        AccessibilityNodeInfo preview = awaitAny(described("拍照识字相机预览"));
        android.graphics.Rect previewBounds = new android.graphics.Rect();
        preview.getBoundsInScreen(previewBounds);
        Bitmap frame = automation.takeScreenshot();
        check(frame != null, "independent camera frame is available");
        try { check(previewBounds.height() > frame.getHeight() / 2,
            "camera occupies most of the independent window instead of keyboard height"); }
        finally { frame.recycle(); }
    }

    private void screenshot(String name) throws Exception {
        Bitmap image = automation.takeScreenshot();
        check(image != null, "synthetic product screenshot available");
        try (FileOutputStream output = new FileOutputStream(new File(
                getTargetContext().getExternalFilesDir(null), "ocr-product-" + name + ".png"))) {
            check(image.compress(Bitmap.CompressFormat.PNG, 100, output), "write synthetic screenshot");
        } finally { image.recycle(); }
    }

    private static void check(boolean value, String purpose) { if (!value) throw new AssertionError(purpose); }
}
