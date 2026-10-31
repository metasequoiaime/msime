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
    private final Handler handler = new Handler(Looper.getMainLooper());
    private Callback holding;
    private android.content.BroadcastReceiver finishReceiver;

    @Override protected void onStartListening(Intent intent, Callback callback) {
        if (getSharedPreferences("synthetic-speech", 0).getBoolean("hold", false)) {
            startHold(callback);
            return;
        }
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
    /** 长按专项只回传合成文字与音量，不访问麦克风；广播模拟服务先于用户结束。 */
    private void startHold(Callback callback) {
        clearHold();
        holding = callback;
        finishReceiver = new android.content.BroadcastReceiver() {
            @Override public void onReceive(android.content.Context context, Intent intent) {
                finishHold();
            }
        };
        android.content.IntentFilter filter = new android.content.IntentFilter("app.msime.android.test.FINISH_SPEECH");
        if (android.os.Build.VERSION.SDK_INT >= 33)
            registerReceiver(finishReceiver, filter, android.content.Context.RECEIVER_EXPORTED);
        else registerReceiver(finishReceiver, filter);
        try {
            callback.readyForSpeech(new Bundle());
        } catch (RemoteException disconnected) {
            clearHold();
            return;
        }
        handler.post(new Runnable() {
            @Override public void run() {
                if (holding != callback) return;
                try {
                    callback.rmsChanged(6f);
                    callback.partialResults(result());
                    handler.postDelayed(this, 200);
                } catch (RemoteException disconnected) {
                    clearHold();
                }
            }
        });
    }

    private static Bundle result() {
        Bundle result = new Bundle();
        result.putStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION,
            new java.util.ArrayList<>(java.util.List.of("合成语音")));
        return result;
    }

    private void finishHold() {
        Callback callback = holding;
        clearHold();
        if (callback == null) return;
        // 留出编辑器在发送之后变化的真实异步窗口。
        handler.postDelayed(() -> {
            try { callback.endOfSpeech(); callback.results(result()); }
            catch (RemoteException disconnected) { /* 输入法已取消时不再投递。 */ }
        }, 1000);
    }

    private void clearHold() {
        holding = null;
        handler.removeCallbacksAndMessages(null);
        if (finishReceiver != null) unregisterReceiver(finishReceiver);
        finishReceiver = null;
    }

    @Override protected void onStopListening(Callback callback) { finishHold(); }
    @Override protected void onCancel(Callback callback) { clearHold(); }
    @Override public void onDestroy() { clearHold(); super.onDestroy(); }
}
