#import <Foundation/Foundation.h>
#include <cassert>
#include <cstdlib>
#include <cstring>

extern "C" char *msime_macos_pick_file_with(const char *(*picker)(void));
extern "C" char *msime_macos_pick_file(void);
extern "C" char *msime_macos_pick_directory(void);
extern "C" char *msime_macos_pick_voice_model_directory(void);
extern "C" void msime_macos_free_picked_path(char *path);
extern "C" char *msime_macos_pick_application(void);
extern "C" char *msime_macos_bundle_identifier_at(const char *path);
extern "C" char *msime_macos_display_name_at(const char *path);
extern "C" char *msime_macos_application_name_for_bundle_identifier(const char *identifier);

static const char *ChosePath(void) { return "/Users/someone/models/ggml-base.bin"; }
static const char *ChoseNothing(void) { return nullptr; }
static const char *ChoseEmpty(void) { return ""; }
static const char *ChoseAutoreleased(void) {
    return [[@"/tmp" stringByAppendingPathComponent:@"model.bin"] UTF8String];
}

int main() {
    @autoreleasepool {
        char *path = msime_macos_pick_file_with(ChosePath);
        assert(path && strcmp(path, "/Users/someone/models/ggml-base.bin") == 0);
        msime_macos_free_picked_path(path);

        // Cancelling is not a failure and must not produce an empty path the caller would store.
        assert(msime_macos_pick_file_with(ChoseNothing) == nullptr);
        assert(msime_macos_pick_file_with(ChoseEmpty) == nullptr);
        assert(msime_macos_pick_file_with(nullptr) == nullptr);

        // The panel's path lives in an autorelease pool that ends with the call; the result has to be a
        // copy, or the caller reads freed memory.
        char *copied = msime_macos_pick_file_with(ChoseAutoreleased);
        assert(copied && strcmp(copied, "/tmp/model.bin") == 0);
        msime_macos_free_picked_path(copied);

        // Freeing nothing is allowed, so callers need no null check of their own.
        msime_macos_free_picked_path(nullptr);
    }
    {
        // AppKit cannot show a panel off the main thread, so the entry point refuses rather than trying.
        __block char *offMain = reinterpret_cast<char *>(1);
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            offMain = msime_macos_pick_file();
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        assert(offMain == nullptr);
        offMain = reinterpret_cast<char *>(1);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            offMain = msime_macos_pick_directory();
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        assert(offMain == nullptr);
        offMain = reinterpret_cast<char *>(1);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            offMain = msime_macos_pick_voice_model_directory();
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        assert(offMain == nullptr);
        offMain = reinterpret_cast<char *>(1);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            offMain = msime_macos_pick_application();
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        assert(offMain == nullptr);
    }
    {
        // 应用例外：选中的应用按路径读 Bundle ID 和名称，已有规则按 Bundle ID 查名称。计算器在每台 Mac 上都在同一个位置。
        char *identifier = msime_macos_bundle_identifier_at("/System/Applications/Calculator.app");
        assert(identifier && strcmp(identifier, "com.apple.calculator") == 0);
        msime_macos_free_picked_path(identifier);
        char *name = msime_macos_display_name_at("/System/Applications/Calculator.app");
        assert(name && name[0] != '\0');
        char *installed = msime_macos_application_name_for_bundle_identifier("com.apple.calculator");
        assert(installed && strcmp(installed, name) == 0);
        msime_macos_free_picked_path(installed);
        msime_macos_free_picked_path(name);
        // 不是应用程序的目录、不存在的路径、没装的应用都没有答案，调用方据此报错或照旧显示 Bundle ID。
        assert(msime_macos_bundle_identifier_at("/System") == nullptr);
        assert(msime_macos_bundle_identifier_at("/nonexistent/Nothing.app") == nullptr);
        assert(msime_macos_bundle_identifier_at(nullptr) == nullptr);
        assert(msime_macos_display_name_at("/nonexistent/Nothing.app") == nullptr);
        assert(msime_macos_application_name_for_bundle_identifier("app.msime.tests.not-installed") == nullptr);
        assert(msime_macos_application_name_for_bundle_identifier("") == nullptr);
    }
    return 0;
}
