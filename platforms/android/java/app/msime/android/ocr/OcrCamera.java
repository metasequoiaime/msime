package app.msime.android;

import android.hardware.Camera;
import android.os.Handler;
import android.os.HandlerThread;
import android.view.Surface;
import android.view.SurfaceHolder;
import java.util.Comparator;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.BiConsumer;

/** 相机只属于当前面板，所有驱动操作串行放在专用线程上；关闭后拒绝迟到回调。 */
@SuppressWarnings("deprecation")
final class OcrCamera implements AutoCloseable {
    private final HandlerThread thread = new HandlerThread("msime-ocr-camera");
    private final Handler worker;
    private volatile boolean closed;
    private Camera camera;

    OcrCamera() {
        thread.start();
        worker = new Handler(thread.getLooper());
    }

    void start(SurfaceHolder surface, int rotation, BiConsumer<Integer, Integer> ready, Runnable failure) {
        worker.post(() -> {
            if (closed) return;
            try {
                int id = -1;
                Camera.CameraInfo info = new Camera.CameraInfo();
                for (int i = 0; i < Camera.getNumberOfCameras(); i++) {
                    Camera.getCameraInfo(i, info);
                    if (info.facing == Camera.CameraInfo.CAMERA_FACING_BACK) { id = i; break; }
                }
                if (id < 0) throw new IllegalStateException("ocr_camera_missing");
                camera = Camera.open(id);
                camera.setErrorCallback((code, source) -> { release(); if (!closed) failure.run(); });
                int degrees = switch (rotation) {
                    case Surface.ROTATION_90 -> 90;
                    case Surface.ROTATION_180 -> 180;
                    case Surface.ROTATION_270 -> 270;
                    default -> 0;
                };
                int orientation = (info.orientation - degrees + 360) % 360;
                camera.setDisplayOrientation(orientation);
                Camera.Parameters parameters = camera.getParameters();
                Camera.Size picture = parameters.getSupportedPictureSizes().stream()
                    .filter(size -> size.width <= 2560 && size.height <= 2560)
                    .max(Comparator.comparingLong(size -> (long) size.width * size.height))
                    .orElseGet(() -> parameters.getSupportedPictureSizes().stream()
                        .min(Comparator.comparingLong(size -> (long) size.width * size.height)).orElseThrow());
                parameters.setPictureSize(picture.width, picture.height);
                // 预览和照片使用同一宽高比，避免用户框选范围与照片裁剪范围不一致。
                double pictureRatio = (double) picture.width / picture.height;
                Camera.Size previewSize = parameters.getSupportedPreviewSizes().stream()
                    .min(Comparator.<Camera.Size>comparingDouble(size ->
                        Math.abs((double) size.width / size.height - pictureRatio))
                        .thenComparingLong(size -> Math.abs((long) size.width * size.height - 1280L * 960)))
                    .orElseThrow();
                parameters.setPreviewSize(previewSize.width, previewSize.height);
                parameters.setRotation(orientation);
                List<String> modes = parameters.getSupportedFocusModes();
                if (modes != null && modes.contains(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE))
                    parameters.setFocusMode(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE);
                camera.setParameters(parameters);
                camera.setPreviewDisplay(surface);
                Camera.Size preview = camera.getParameters().getPreviewSize();
                int width = orientation % 180 == 0 ? preview.width : preview.height;
                int height = orientation % 180 == 0 ? preview.height : preview.width;
                // 驱动接受 startPreview 不代表首帧已到；收到首帧后才允许拍照。
                camera.setOneShotPreviewCallback((bytes, source) -> {
                    if (closed) release(); else ready.accept(width, height);
                });
                camera.startPreview();
            } catch (Exception | LinkageError unavailable) { release(); if (!closed) failure.run(); }
        });
    }

    void capture(Consumer<byte[]> success, Runnable failure) {
        worker.post(() -> {
            if (closed || camera == null) { if (!closed) failure.run(); return; }
            try {
                camera.takePicture(null, null, (bytes, source) -> {
                    release();
                    if (!closed) success.accept(bytes);
                });
            } catch (RuntimeException unavailable) { release(); if (!closed) failure.run(); }
        });
    }

    private void release() {
        if (camera == null) return;
        try { camera.stopPreview(); } catch (RuntimeException ignored) { }
        try { camera.release(); } catch (RuntimeException ignored) { }
        camera = null;
    }

    @Override public void close() {
        if (closed) return;
        closed = true;
        worker.post(() -> { release(); thread.quitSafely(); });
    }
}
