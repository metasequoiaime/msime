//! 语音贡献：`POST /v1/voice/contributions`，multipart 请求体是一个 `payload`（JSON `{language,provider,duration_ms,transcript,app_version}`）加一个 `audio`（msime-cloud `internal/server/voice_contributions.go`）。需要用户会话，匿名账号也可以；成功返回 201 `{id}`。服务端每个用户每小时 30 次，样本保存 180 天，注销账号或删除云端数据的 `voice` 分区时一并删除。
//!
//! 只收 WAV：服务端对 WAV 做 RIFF 结构校验（`internal/server/wav.go` 的 `validWAV`），这里按同一套规则检查，再从 `fmt ` 的字节率和 `data` 的大小算出时长作为 `duration_ms`。服务端不解析时长，要求客户端按音频本身确认不超过 60 秒，所以时长由这里算，不由宿主声明。服务端还收 Ogg，但不解析就算不出时长，这里不收。
//!
//! `language`、`provider`、`app_version` 是服务端的 `^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$`；识别文本去掉首尾空白后发送，非空、最多 2000 个码点，控制字符只允许 `\n` 和 `\t`。音频最多 2 MiB。

use super::*;

pub const VOICE_CONTRIBUTIONS_PATH: &str = "/v1/voice/contributions";
/// 一段音频的上限（服务端 `voiceContributionAudioBytes`）。
pub const MAX_VOICE_CONTRIBUTION_AUDIO_BYTES: usize = 2 << 20;
/// 时长上限（服务端 `voiceContributionMaxMS`）。
pub const MAX_VOICE_CONTRIBUTION_MILLIS: u64 = 60_000;
/// 识别文本的码点上限（服务端 `voiceContributionTranscriptRunes`）。
const MAX_VOICE_TRANSCRIPT_CHARS: usize = 2000;
/// `payload` 的上限（服务端 `voiceContributionPayloadBytes`）。
const MAX_VOICE_PAYLOAD_BYTES: usize = 16 << 10;
/// 上传 2 MiB 的时限，与反馈相同。
const VOICE_CONTRIBUTION_TIMEOUT: Duration = Duration::from_secs(60);

/// 宿主给的一次贡献：识别语言、识别器、识别文本、应用版本和 WAV 音频的字节。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountVoiceContributionInput {
    pub language: String,
    pub provider: String,
    pub transcript: String,
    pub app_version: String,
    pub audio: Vec<u8>,
}

/// 校验过、可以发送的贡献。用 [`prepare_voice_contribution`] 构造。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountVoiceContribution {
    payload: Vec<u8>,
    audio: Vec<u8>,
    duration_ms: u64,
}

/// 服务端收下贡献后给的 id（64 位小写十六进制）。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountVoiceContributionReceipt {
    pub id: String,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// 按服务端 `validWAV` 的规则检查 WAV，合规时返回 `data` 的字节数和 `fmt ` 的字节率。规则：RIFF 头声明的大小正好是整个文件；块逐个排到文件末尾，奇数大小的块后面有一个填充字节；恰好一个 `fmt `（至少 16 字节，PCM 8/16/24/32 位或浮点 32/64 位，1–8 声道，8000–192000 Hz，块对齐和字节率与之一致）和恰好一个非空的 `data`，`data` 的大小是块对齐的整数倍。
fn wav_data(audio: &[u8]) -> Option<(u32, u32)> {
    if audio.len() < 44
        || &audio[..4] != b"RIFF"
        || &audio[8..12] != b"WAVE"
        || u64::from(u32_at(audio, 4)) + 8 != audio.len() as u64
    {
        return None;
    }
    let mut format = None;
    let mut data = None;
    let mut offset = 12;
    while offset < audio.len() {
        if audio.len() - offset < 8 {
            return None;
        }
        let name = &audio[offset..offset + 4];
        let size = u32_at(audio, offset + 4) as usize;
        offset += 8;
        if size > audio.len() - offset {
            return None;
        }
        let chunk = &audio[offset..offset + size];
        match name {
            b"fmt " => {
                if format.is_some() || size < 16 {
                    return None;
                }
                let encoding = u16_at(chunk, 0);
                let channels = u16_at(chunk, 2);
                let rate = u32_at(chunk, 4);
                let byte_rate = u32_at(chunk, 8);
                let align = u16_at(chunk, 12);
                let bits = u16_at(chunk, 14);
                let valid_bits = (encoding == 1 && matches!(bits, 8 | 16 | 24 | 32))
                    || (encoding == 3 && matches!(bits, 32 | 64));
                if !valid_bits
                    || !(1..=8).contains(&channels)
                    || !(8000..=192_000).contains(&rate)
                    || align != channels * (bits / 8)
                    || u64::from(byte_rate) != u64::from(rate) * u64::from(align)
                {
                    return None;
                }
                format = Some((align, byte_rate));
            }
            b"data" => {
                if data.is_some() || size == 0 {
                    return None;
                }
                data = Some(size as u32);
            }
            _ => {}
        }
        offset += size;
        if !size.is_multiple_of(2) {
            if offset >= audio.len() {
                return None;
            }
            offset += 1;
        }
    }
    let (align, byte_rate) = format?;
    let data = data?;
    (align > 0 && data.is_multiple_of(u32::from(align))).then_some((data, byte_rate))
}

/// WAV 的时长（毫秒，向下取整）；不合规时为 `None`。
fn wav_duration_ms(audio: &[u8]) -> Option<u64> {
    let (data, byte_rate) = wav_data(audio)?;
    Some(u64::from(data) * 1000 / u64::from(byte_rate))
}

/// 按服务端的规则校验一次贡献，算出时长，整理成要发的 `payload`。音频超过 2 MiB 是 `account_too_large`；其余不合规（字段形状不对、识别文本为空或太长、不是合规的 WAV、时长不在 1–60000 毫秒）是 `account_invalid`。
pub fn prepare_voice_contribution(
    input: &AccountVoiceContributionInput,
) -> Result<AccountVoiceContribution, AccountFailure> {
    use crate::diagnostics::upload::valid_diagnostic_label;
    if input.audio.len() > MAX_VOICE_CONTRIBUTION_AUDIO_BYTES {
        return Err(AccountFailure::too_large());
    }
    let transcript = input.transcript.trim();
    if !valid_diagnostic_label(&input.language)
        || !valid_diagnostic_label(&input.provider)
        || !valid_diagnostic_label(&input.app_version)
        || transcript.is_empty()
        || transcript.chars().count() > MAX_VOICE_TRANSCRIPT_CHARS
        || crate::has_disallowed_control_with_allowed(transcript, &['\n', '\t'])
    {
        return Err(AccountError::Invalid.into());
    }
    let duration_ms = wav_duration_ms(&input.audio)
        .filter(|duration| (1..=MAX_VOICE_CONTRIBUTION_MILLIS).contains(duration))
        .ok_or(AccountError::Invalid)?;
    #[derive(Serialize)]
    struct Payload<'a> {
        language: &'a str,
        provider: &'a str,
        duration_ms: u64,
        transcript: &'a str,
        app_version: &'a str,
    }
    let payload = serde_json::to_vec(&Payload {
        language: &input.language,
        provider: &input.provider,
        duration_ms,
        transcript,
        app_version: &input.app_version,
    })
    .map_err(|_| AccountError::Invalid)?;
    if payload.len() > MAX_VOICE_PAYLOAD_BYTES {
        return Err(AccountFailure::too_large());
    }
    Ok(AccountVoiceContribution {
        payload,
        audio: input.audio.clone(),
        duration_ms,
    })
}

impl AccountVoiceContribution {
    /// 要发的 `payload` 部分（JSON），测试和自己发 multipart 的宿主看它。
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// 从 WAV 算出的时长（毫秒），也就是 `payload` 里的 `duration_ms`。
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

impl BackendAccountClient {
    pub(super) fn send_voice_contribution(
        &self,
        contribution: &AccountVoiceContribution,
        access_token: &str,
    ) -> Result<AccountVoiceContributionReceipt, AccountFailure> {
        use reqwest::blocking::multipart::{Form, Part};
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let payload = Part::bytes(contribution.payload.clone())
            .mime_str("application/json")
            .map_err(|_| AccountError::Invalid)?;
        let audio = Part::bytes(contribution.audio.clone())
            .file_name("voice.wav")
            .mime_str("audio/wav")
            .map_err(|_| AccountError::Invalid)?;
        let form = Form::new().part("payload", payload).part("audio", audio);
        let response = self
            .http()?
            .post(self.url(VOICE_CONTRIBUTIONS_PATH)?)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(access_token)
            .timeout(VOICE_CONTRIBUTION_TIMEOUT)
            .multipart(form)
            .send()
            .map_err(|error| AccountFailure::from_send_error(&error))?;
        let bytes = read_detailed_response(response, MAX_JSON_BYTES)?;
        let receipt: AccountVoiceContributionReceipt =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        if !crate::text::is_lower_hex(&receipt.id, 64) {
            return Err(AccountError::Unavailable.into());
        }
        Ok(receipt)
    }
}

/// 合成的 PCM WAV：`channels` 声道、`rate` Hz、`bits` 位，`data` 有 `data_bytes` 字节（全零）。测试用。
#[cfg(test)]
pub(crate) fn synthetic_wav(channels: u16, rate: u32, bits: u16, data_bytes: u32) -> Vec<u8> {
    let align = channels * (bits / 8);
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    wav.extend_from_slice(&align.to_le_bytes());
    wav.extend_from_slice(&bits.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    wav.resize(wav.len() + data_bytes as usize, 0);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// 16 kHz 单声道 16 位，一秒是 32000 字节。
    fn input(data_bytes: u32) -> AccountVoiceContributionInput {
        AccountVoiceContributionInput {
            language: "zh-CN".into(),
            provider: "system".into(),
            transcript: "  合成识别文本\n第二行  ".into(),
            app_version: "0.9.0".into(),
            audio: synthetic_wav(1, 16_000, 16, data_bytes),
        }
    }

    #[test]
    fn the_duration_comes_from_the_wav_and_the_transcript_is_trimmed() {
        let contribution = prepare_voice_contribution(&input(32_000 * 3 / 2)).unwrap();
        assert_eq!(contribution.duration_ms(), 1500);
        let payload: Value = serde_json::from_slice(contribution.payload()).unwrap();
        assert_eq!(
            payload,
            serde_json::json!({
                "language": "zh-CN",
                "provider": "system",
                "duration_ms": 1500,
                "transcript": "合成识别文本\n第二行",
                "app_version": "0.9.0",
            })
        );
        // 刚好 60 秒可以，多一个采样就不行。
        assert_eq!(
            prepare_voice_contribution(&input(32_000 * 60))
                .unwrap()
                .duration_ms(),
            60_000
        );
        assert_eq!(
            prepare_voice_contribution(&input(32_000 * 60 + 32))
                .unwrap_err()
                .code(),
            "account_invalid"
        );
        // 不到 1 毫秒。
        assert_eq!(
            prepare_voice_contribution(&input(2)).unwrap_err().code(),
            "account_invalid"
        );
    }

    #[test]
    fn fields_follow_the_server_rules() {
        let invalid = |change: &dyn Fn(&mut AccountVoiceContributionInput)| {
            let mut value = input(32_000);
            change(&mut value);
            prepare_voice_contribution(&value).unwrap_err().code()
        };
        assert_eq!(
            invalid(&|value| value.language = "".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.language = "-zh".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.provider = "a".repeat(65)),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.app_version = "0.9 beta".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.transcript = " \n ".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.transcript = "回车\r换行".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.transcript = "字".repeat(2001)),
            "account_invalid"
        );
        let mut longest = input(32_000);
        longest.transcript = "字".repeat(2000);
        assert!(prepare_voice_contribution(&longest).is_ok());
        let mut large = input(32_000);
        large
            .audio
            .resize(MAX_VOICE_CONTRIBUTION_AUDIO_BYTES + 1, 0);
        assert_eq!(
            prepare_voice_contribution(&large).unwrap_err().code(),
            "account_too_large"
        );
    }

    #[test]
    fn the_wav_is_checked_like_the_server_does() {
        let good = synthetic_wav(2, 48_000, 24, 6 * 100);
        assert_eq!(wav_data(&good), Some((600, 48_000 * 6)));
        let broken = |change: &dyn Fn(&mut Vec<u8>)| {
            let mut wav = synthetic_wav(1, 16_000, 16, 3200);
            change(&mut wav);
            wav_data(&wav)
        };
        // RIFF 头声明的大小与文件不符。
        assert_eq!(broken(&|wav| wav.push(0)), None);
        // 不是 RIFF / WAVE。
        assert_eq!(broken(&|wav| wav[0] = b'X'), None);
        assert_eq!(broken(&|wav| wav[8] = b'X'), None);
        // 压缩格式（编码 2）。
        assert_eq!(broken(&|wav| wav[20] = 2), None);
        // 采样率低于 8000。
        assert_eq!(
            broken(&|wav| wav[24..28].copy_from_slice(&7_999_u32.to_le_bytes())),
            None
        );
        // 字节率与采样率、块对齐不一致。
        assert_eq!(broken(&|wav| wav[28] ^= 1), None);
        // `data` 不是块对齐的整数倍：去掉最后一个字节并改正两处大小。
        assert_eq!(
            broken(&|wav| {
                wav.pop();
                let riff = (wav.len() - 8) as u32;
                wav[4..8].copy_from_slice(&riff.to_le_bytes());
                wav[40..44].copy_from_slice(&3199_u32.to_le_bytes());
                // 奇数大小的块后面要有填充字节。
                wav.push(0);
                let riff = (wav.len() - 8) as u32;
                wav[4..8].copy_from_slice(&riff.to_le_bytes());
            }),
            None
        );
        // 没有 `data`。
        assert_eq!(
            broken(&|wav| {
                wav[36..40].copy_from_slice(b"junk");
            }),
            None
        );
        // 未知的块跳过；奇数大小的块带填充字节时照样合规。
        let mut padded = synthetic_wav(1, 16_000, 16, 3200);
        padded.extend_from_slice(b"LIST");
        padded.extend_from_slice(&3_u32.to_le_bytes());
        padded.extend_from_slice(b"abc\0");
        let riff = (padded.len() - 8) as u32;
        padded[4..8].copy_from_slice(&riff.to_le_bytes());
        assert_eq!(wav_data(&padded), Some((3200, 32_000)));
        // 同样的块少了填充字节。
        padded.pop();
        let riff = (padded.len() - 8) as u32;
        padded[4..8].copy_from_slice(&riff.to_le_bytes());
        assert_eq!(wav_data(&padded), None);
        // Ogg 不收。
        let mut ogg = b"OggS\0".to_vec();
        ogg.resize(64, 0);
        assert_eq!(wav_data(&ogg), None);
    }
}
