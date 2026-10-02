#import "VoiceFailureMessages.h"
#include "../../../../shared/voice/VoiceProviders.h"
#include <cassert>
#include <string>

// The sentences are MSIME-Windows' own (voice_input_service.cpp Recognize, doubao_asr_client.cpp Run); the fixtures are synthetic.
int main() {
    @autoreleasepool {
        using msime::voice::cloud_asr_error_detail;
        using msime::voice::cloud_asr_status_message;
        assert(cloud_asr_error_detail(R"({"error":"synthetic string error"})") == "synthetic string error");
        assert(cloud_asr_error_detail(R"({"error":{"message":"Incorrect synthetic key","type":"invalid_request_error"}})") == "Incorrect synthetic key");
        assert(cloud_asr_error_detail(R"({"message":"synthetic quota","code":20015,"data":"synthetic data"})") == "synthetic quota（code 20015） synthetic data");
        assert(cloud_asr_error_detail(R"({"message":"synthetic quota","code":"E1"})") == "synthetic quota（code \"E1\"）");
        assert(cloud_asr_error_detail(R"({"message":"synthetic quota","code":null,"data":""})") == "synthetic quota");
        assert(cloud_asr_error_detail("") == "");
        assert(cloud_asr_error_detail("synthetic plain body") == "synthetic plain body");
        assert(cloud_asr_error_detail(R"(["not an object"])") == R"(["not an object"])");
        const std::string ascii(300, 'x');
        assert(cloud_asr_error_detail(ascii) == std::string(240, 'x') + "...");
        // 239 ASCII bytes and then a three-byte character straddling the 240-byte cut: the character is dropped whole, not split.
        const std::string straddling = std::string(239, 'x') + "界" + std::string(40, 'y');
        assert(cloud_asr_error_detail(straddling) == std::string(239, 'x') + "...");

        assert(cloud_asr_status_message(401, R"({"error":{"message":"Incorrect synthetic key"}})", "openai", "whisper-1", "") ==
               "语音识别失败：Incorrect synthetic key");
        assert(cloud_asr_status_message(404, "", "groq", "fixture", "") == "语音识别失败：HTTP 404");
        // Only SiliconFlow's 5xx gets the explanation, and the trace id only when the service sent one.
        assert(cloud_asr_status_message(503, "", "openai", "fixture", "synthetic-trace") == "语音识别失败：HTTP 503");
        assert(cloud_asr_status_message(500, "", "siliconflow", "FunAudioLLM/SenseVoiceSmall", "synthetic-trace") ==
               "语音识别失败：HTTP 500。这是硅基流动服务端内部错误，模型名 FunAudioLLM/SenseVoiceSmall 本身是官方支持的。 追踪 ID：synthetic-trace。");
        assert(cloud_asr_status_message(502, "", "SiliconFlow", "fixture", "") ==
               "语音识别失败：HTTP 502。这是硅基流动服务端内部错误，模型名 fixture 本身是官方支持的。");
        assert(cloud_asr_status_message(429, "", "siliconflow", "fixture", "synthetic-trace") == "语音识别失败：HTTP 429");
        assert(msime::voice::cloud_asr_transport_message("Could not resolve host: synthetic.invalid") ==
               "语音识别请求失败：Could not resolve host: synthetic.invalid");
        const msime::voice::CloudAsrError error("Voice HTTP status 401", "语音识别失败：synthetic");
        assert(std::string(error.what()) == "Voice HTTP status 401" && error.user_message() == "语音识别失败：synthetic");

        assert([MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureConnect, YES, 0) isEqual:@"无法连接豆包语音识别。请检查 App ID、Access Token 和接口地址。"]);
        assert([MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureConnect, NO, 0) isEqual:@"无法连接豆包语音识别。请检查 API Key 和接口地址。"]);
        assert([MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureHandshake, NO, 0) isEqual:@"豆包语音识别握手失败。"]);
        assert([MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureServerCode, NO, 45000001) isEqual:@"豆包语音识别失败（code 45000001）。请检查 Access Token。"]);
        assert([MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureServerCode, YES, -1) isEqual:@"豆包语音识别失败（code -1）。请检查 Access Token。"]);

        NSDictionary *info = @{NSLocalizedFailureReasonErrorKey: @"synthetic detail"};
        assert([MSIMEVoiceFailureDetail([NSError errorWithDomain:@"app.msime.client.voice" code:6 userInfo:info]) isEqual:@"synthetic detail"]);
        assert([MSIMEVoiceFailureDetail([NSError errorWithDomain:@"app.msime.client.voice.doubao" code:1 userInfo:info]) isEqual:@"synthetic detail"]);
        assert(!MSIMEVoiceFailureDetail([NSError errorWithDomain:NSURLErrorDomain code:-1004 userInfo:info]));
        assert(!MSIMEVoiceFailureDetail([NSError errorWithDomain:@"app.msime.client.voice" code:6 userInfo:@{NSLocalizedFailureReasonErrorKey: @""}]));
        assert(!MSIMEVoiceFailureDetail([NSError errorWithDomain:@"app.msime.client.voice" code:6 userInfo:@{NSLocalizedFailureReasonErrorKey: @3}]));
        assert(!MSIMEVoiceFailureDetail(nil));
    }
    return 0;
}
