use crate::shared::voice::voice_sessions::VoiceSessions;
use crate::voice::{VoiceRecognitionRequest, VoiceRecognitionResult, VoiceRecognitionUpdate};
use crate::HostActionError;
use msime_client_core::voice::controller::{Error, Phase};
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

pub(crate) async fn recognize(
    app: tauri::AppHandle,
    request: VoiceRecognitionRequest,
) -> Result<VoiceRecognitionResult, HostActionError> {
    let sessions = app.state::<VoiceSessions>();
    // 本版本 Server 的语音控制管道；安装包的版本声明坏了时没有可连的 Server。
    let pipe = msime_host_windows::voice_controller::pipe_name().ok_or(HostActionError {
        code: "unavailable",
    })?;
    let session = sessions
        .begin(request.request_id, std::path::PathBuf::from(pipe))
        .ok_or(HostActionError { code: "busy" })?;
    let generation = session.generation;
    let cancelled = session.cancelled.clone();
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        msime_host_windows::voice_controller::recognize(
            &request.language,
            generation,
            &session.stopped,
            &session.cancelled,
            |value| {
                if session.cancelled.load(Ordering::Acquire) {
                    return;
                }
                let phase = match value.phase {
                    Phase::Recording => Some("recording"),
                    Phase::Recognizing => Some("recognizing"),
                    Phase::Processing => Some("polishing"),
                    _ => None,
                };
                let _ = worker_app.emit(
                    "voice-update",
                    VoiceRecognitionUpdate {
                        request_id: session.request_id.clone(),
                        text: value.text.clone(),
                        final_result: value.phase == Phase::Complete,
                        phase: phase.map(str::to_owned),
                        level: (value.phase == Phase::Recording).then_some(value.level),
                    },
                );
            },
        )
    })
    .await;
    sessions.finish(generation);
    if cancelled.load(Ordering::Acquire) {
        return Err(HostActionError { code: "cancelled" });
    }
    let text = result
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .map_err(|error| HostActionError {
            code: match error {
                Error::Busy => "busy",
                Error::Cancelled => "cancelled",
                Error::Invalid => "invalid_voice",
                Error::Stale => "stale",
                Error::Denied | Error::Unavailable => "unavailable",
            },
        })?;
    Ok(VoiceRecognitionResult { text })
}
