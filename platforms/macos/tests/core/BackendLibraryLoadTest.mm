#import <Foundation/Foundation.h>
#include <dlfcn.h>
#include <cassert>
#include "MSIMERustClient.h"

// 只检查 dylib 收不收这张表，表里的函数不会被调用，这里不链接 Rust 宿主库。
static void MSIMETestStringFree(char *) {}
static char *MSIMETestRegister(const msime_client_session_store *) { return nullptr; }
static char *MSIMETestUnregister(uint64_t) { return nullptr; }

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        assert(argc == 2);
        void *library = dlopen(argv[1], RTLD_NOW | RTLD_GLOBAL);
        assert(library);
        Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
        assert(bridge && [bridge respondsToSelector:NSSelectorFromString(@"shared")]);
        for (NSString *selector in @[@"showDictionaryForAccountID:", @"showClipboardForAccountID:",
                                    @"showSnapshotForAccountID:", @"showSettingsForAccountID:",
                                    @"showCommunityResourcesForAccountID:", @"showHandwriting", @"showHandwritingWithSelectionAttempt:",
                                    @"showEmojiWithOptions:selectionAttempt:", @"applyEmojiPreferences:",
                                    @"showEmojiDeliveryFailure", @"startClipboardCaptureWithOptions:",
                                    @"stopClipboardCapture"]) {
            assert([bridge instancesRespondToSelector:NSSelectorFromString(selector)]);
        }
        Class account = NSClassFromString(@"MSIMEBackendAccountWindow");
        assert(account && [account respondsToSelector:@selector(shared)]);
        assert([account instancesRespondToSelector:@selector(showAccount)]);
        assert([account instancesRespondToSelector:@selector(showCloudClipboard)]);
        Class clipboard = NSClassFromString(@"MSIMEBackendCloudClipboardProvider");
        assert(clipboard && [clipboard respondsToSelector:NSSelectorFromString(@"prepareWithCompletion:")]);
        assert([clipboard instancesRespondToSelector:NSSelectorFromString(@"request:completion:")]);
        Class dictionary = NSClassFromString(@"MSIMEBackendCloudDictionaryProvider");
        assert(dictionary && [dictionary respondsToSelector:NSSelectorFromString(@"prepareWithCompletion:")]);
        assert([dictionary instancesRespondToSelector:NSSelectorFromString(@"request:completion:")]);
        // 输入法经 MSIMEInstallRustClient 把 Rust 函数表交给 dylib：合格的表收下，版本或大小不对的不收。
        using MSIMEInstallRustClient = bool (*)(const msime_rust_client *);
        auto install = reinterpret_cast<MSIMEInstallRustClient>(dlsym(library, "MSIMEInstallRustClient"));
        assert(install);
        msime_rust_client table = {};
        table.version = MSIME_RUST_CLIENT_VERSION;
        table.size = sizeof(msime_rust_client);
        table.string_free = MSIMETestStringFree;
        table.session_store_register = MSIMETestRegister;
        table.session_store_unregister = MSIMETestUnregister;
        assert(install(&table));
        msime_rust_client newer = table;
        newer.version = MSIME_RUST_CLIENT_VERSION + 1;
        assert(!install(&newer));
        msime_rust_client shorter = table;
        shorter.size = sizeof(msime_rust_client) - sizeof(void *);
        assert(!install(&shorter));
        msime_rust_client incomplete = table;
        incomplete.session_store_register = nullptr;
        assert(!install(&incomplete));
        assert(!install(nullptr));
        // Objective-C classes remain registered; retain the library for process lifetime.
    }
    return 0;
}
