#import "../../src/core/TypingKeyPressRequest.h"
#include "msime_client.h"
#include <cassert>
#include <cstring>

using msime::mac::KeyPressBatch;
using msime::mac::TypingSource;
using msime::mac::KeyPressRequest;

static NSDictionary *Call(NSDictionary *request) {
    NSData *encoded = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    assert(encoded);
    char *raw = msime_client_typing_statistics(static_cast<const uint8_t *>(encoded.bytes), encoded.length);
    assert(raw);
    NSData *body = [NSData dataWithBytes:raw length:strlen(raw)];
    msime_client_string_free(raw);
    NSDictionary *result = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
    assert([result isKindOfClass:NSDictionary.class]);
    return result;
}

static void CaptureBeforeDispatch() {
    int reads = 0;
    TypingSource mode = TypingSource::Japanese;
    auto resolve = [&] { ++reads; return mode; };
    using msime::mac::CaptureKeyPressSource;
    assert(CaptureKeyPressSource(false, false, resolve) == TypingSource::Unknown);
    assert(CaptureKeyPressSource(false, true, resolve) == TypingSource::Unknown);
    assert(CaptureKeyPressSource(true, true, resolve) == TypingSource::English);
    assert(reads == 0);
    const auto captured = CaptureKeyPressSource(true, false, resolve);
    mode = TypingSource::English;
    assert(captured == TypingSource::Japanese && reads == 1);
    assert(CaptureKeyPressSource(true, false, resolve) == TypingSource::English);
    assert(reads == 2);
}

static void BatchBoundariesAndEncoding() {
    KeyPressBatch batch;
    assert(batch.record("2026-11-17", "KeyA", TypingSource::Quanpin).empty());
    assert(batch.record("2026-11-17", "KeyA", TypingSource::English).empty());
    assert(batch.record("2026-11-17", "KeyB", TypingSource::Japanese).empty());
    assert(batch.record("2026-11-17", "KeyC").empty());
    auto first = batch.drain();
    assert(first && batch.empty());
    assert(first->keys.at("KeyA") == 2);
    assert(first->sources.at("quanpin").at("KeyA") == 1);
    assert(first->sources.at("english").at("KeyA") == 1);
    assert(first->sources.at("japanese").at("KeyB") == 1);
    assert(first->sources.at("unknown").at("KeyC") == 1);
    NSDictionary *request = KeyPressRequest(@"/synthetic", *first);
    assert([request[@"action"] count] == 4);
    NSDictionary *expectedKeys = @{@"KeyA": @2, @"KeyB": @1, @"KeyC": @1};
    assert([request[@"action"][@"keys"] isEqual:expectedKeys]);
    NSData *snapshot = [NSJSONSerialization dataWithJSONObject:request options:NSJSONWritingSortedKeys error:nil];
    assert(batch.record("2026-11-17", "KeyA", TypingSource::Korean).empty());
    auto previous = batch.record("2026-11-18", "KeyB", TypingSource::English);
    assert(previous.size() == 1 && previous[0].day == "2026-11-17");
    assert(previous[0].sources.at("korean").at("KeyA") == 1);
    auto next = batch.drain();
    assert(next && next->sources.size() == 1 && next->sources.at("english").at("KeyB") == 1);
    assert([[NSJSONSerialization dataWithJSONObject:KeyPressRequest(@"/synthetic", *first)
        options:NSJSONWritingSortedKeys error:nil] isEqual:snapshot]);
    for (uint64_t count = 1; count < KeyPressBatch::kFlushThreshold; ++count)
        assert(batch.record("2026-11-18", "KeyA", count % 2 ? TypingSource::Quanpin : TypingSource::English).empty());
    auto full = batch.record("2026-11-18", "KeyA", TypingSource::English);
    assert(full.size() == 1 && batch.empty());
    assert(full[0].keys.at("KeyA") == 256);
    assert(full[0].sources.at("quanpin").at("KeyA") == 128);
    assert(full[0].sources.at("english").at("KeyA") == 128);
    assert(batch.record("2026-11-18", "KeyA", TypingSource::Japanese).empty());
    batch.clear();
    assert(batch.empty() && !batch.drain());
    assert(batch.record("2026-11-18", "KeyB").empty());
    auto cleared = batch.drain();
    assert(cleared->sources.size() == 1 && cleared->sources.at("unknown").at("KeyB") == 1);
}

static void SharedStore() {
    NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:directory
        withIntermediateDirectories:YES attributes:nil error:nil]);
    NSDateComponents *date = [NSCalendar.currentCalendar components:NSCalendarUnitYear | NSCalendarUnitMonth |
        NSCalendarUnitDay fromDate:NSDate.date];
    NSString *day = [NSString stringWithFormat:@"%04ld-%02ld-%02ld", (long)date.year, (long)date.month, (long)date.day];
    auto action = [&](NSDictionary *value) { return Call(@{@"directory": directory, @"action": value}); };
    KeyPressBatch batch;
    for (const auto source : {TypingSource::Quanpin, TypingSource::English}) {
        assert(batch.record(day.UTF8String, "KeyA", source).empty());
        assert(batch.record(day.UTF8String, "Tab", source).empty());
    }
    assert(batch.record(day.UTF8String, "KeyB", TypingSource::Japanese).empty());
    assert(batch.record(day.UTF8String, "KeyC").empty());
    auto flush = batch.drain();
    NSDictionary *request = KeyPressRequest(directory, *flush);
    // 来源字段不改变出厂关闭的统计开关。
    assert([Call(request)[@"value"][@"recorded"] unsignedLongLongValue] == 0);
    assert([action(@{@"operation": @"set_enabled", @"enabled": @YES})[@"ok"] boolValue]);
    assert([Call(request)[@"value"][@"recorded"] unsignedLongLongValue] == 6);
    NSDictionary *loaded = action(@{@"operation": @"load"});
    assert([loaded[@"value"][@"dailyKeys"][day] isEqual:request[@"action"][@"keys"]]);
    NSDictionary *languages = loaded[@"value"][@"dailyKeyLanguages"][day];
    assert([languages[@"chinese"][@"KeyA"] intValue] == 1);
    assert([languages[@"english"][@"KeyA"] intValue] == 1);
    assert([languages[@"japanese"][@"KeyB"] intValue] == 1);
    assert([languages[@"unknown"][@"KeyC"] intValue] == 1);
    assert([languages[@"common"][@"Tab"] intValue] == 2);
    // 旧调用保持未知，不用当前模式补历史。
    assert([action(@{@"operation": @"record_keys", @"day": day, @"keys": @{@"KeyA": @1}})[@"ok"] boolValue]);
    loaded = action(@{@"operation": @"load"});
    assert([loaded[@"value"][@"dailyKeyLanguages"][day][@"unknown"][@"KeyA"] intValue] == 1);
    NSMutableDictionary *invalid = [request[@"action"] mutableCopy];
    invalid[@"sources"] = @{@"english": @{@"KeyA": @999}};
    assert(![action(invalid)[@"ok"] boolValue]);
    assert([action(@{@"operation": @"load"}) isEqual:loaded]);
    // 所有现有来源经真实共享层分类，不在宿主重复语言规则。
    for (int raw = 0; raw <= static_cast<int>(TypingSource::SmartAbc); ++raw)
        assert(batch.record(day.UTF8String, "KeyZ", static_cast<TypingSource>(raw)).empty());
    auto all = batch.drain();
    assert([Call(KeyPressRequest(directory, *all))[@"value"][@"recorded"] intValue] ==
        static_cast<int>(TypingSource::SmartAbc) + 1);
    assert([NSFileManager.defaultManager removeItemAtPath:directory error:nil]);
}

int main() {
    @autoreleasepool {
        CaptureBeforeDispatch();
        BatchBoundariesAndEncoding();
        SharedStore();
    }
}
