package app.msime.android.test;

import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.RemoteException;
import android.speech.RecognitionService;
import android.speech.SpeechRecognizer;

/** 先发 ready 再拒绝权限，模拟权限交付检查晚于识别服务启动的回调顺序；不访问麦克风或网络。 */
public class SyntheticSpeechService extends RecognitionService {
    @Override protected void onStartListening(Intent intent, Callback callback) {
        try {
            callback.readyForSpeech(new Bundle());
        } catch (RemoteException disconnected) {
            return;
        }
        new Handler(Looper.getMainLooper()).postDelayed(() -> {
            try {
                callback.error(SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS);
                sendBroadcast(new Intent(VoiceRecognitionLifecycleDeviceSmoke.CALLBACKS)
                    .setPackage("app.msime.android"));
            } catch (RemoteException disconnected) {
                // 测试窗口已关闭时不再投递。
            }
        }, 300);
    }
    @Override protected void onStopListening(Callback callback) { }
    @Override protected void onCancel(Callback callback) { }
}
