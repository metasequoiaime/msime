#!/usr/bin/env python3
"""直接运行原生控制器的请求与取消方法，检查账号与系统翻译等待页的归属。"""
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
compiler = shutil.which("clang++")
if sys.platform != "darwin" or not compiler:
    print("skipped: macOS 候选释义队列归属回归需要 Apple 宿主和 Clang")
    sys.exit(0)

controller = (ROOT / "platforms/macos/src/input/InputController.mm").read_text(encoding="utf-8")


def method(signature):
    definition = re.search(r"^" + re.escape(signature) + r"[^;{]*\{", controller, re.MULTILINE)
    assert definition, f"缺少原生方法定义：{signature}"
    start = definition.start()
    end = controller.index("\n}", start) + 2
    return controller[start:end]


# 提取实际发送和取消的方法；仅替换系统输入法外壳与进程后端，不重写被测行为。
owner = re.search(
    r"^static __weak MSIMEInputController \*MSIMEAccountGlossQueueOwner;$",
    controller, re.MULTILINE,
)
focused = re.search(
    r"^static __weak MSIMEInputController \*MSIMEFocusedController;$",
    controller, re.MULTILINE,
)
assert focused, "缺少原生焦点控制器声明"
declaration = focused.group() + "\n" + (owner.group() if owner else "")
program = '''#import <Foundation/Foundation.h>
#include <cassert>
#include <cstdint>
extern "C" void MSIMEFetchAccountCandidateGlosses(const char *, const char *, const char *, uint64_t)
    __attribute__((weak_import));
extern "C" void MSIMECancelAccountCandidateGlosses(void) __attribute__((weak_import));
extern "C" void MSIMEFetchOnDeviceCandidateGlosses(const char *, const char *)
    __attribute__((weak_import));
extern "C" uint64_t fixturePending(void);
extern "C" void fixtureOnCancel(void (^)(void));
extern "C" NSArray *fixtureDeviceWords(void);
extern "C" NSArray *fixtureDeviceTargets(void);
@interface MSIMEInputController : NSObject
- (void)fetchAccountGlosses:(NSArray<NSString *> *)words primary:(NSString *)primary
                secondary:(NSString *)secondary generation:(uint64_t)generation;
- (void)cancelQueuedAccountGlosses;
- (void)fetchOnDeviceGlosses:(NSArray<NSString *> *)words targets:(NSArray<NSString *> *)targets;
@end
''' + declaration + '\n@implementation MSIMEInputController\n' + method(
    "- (void)fetchAccountGlosses:"
) + '\n' + method("- (void)cancelQueuedAccountGlosses") + '\n' + method(
    "- (void)fetchOnDeviceGlosses:"
) + '''
@end
int main() {
    @autoreleasepool {
        MSIMEInputController *previous = [MSIMEInputController new];
        MSIMEInputController *current = [MSIMEInputController new];
        MSIMEInputController *bystander = [MSIMEInputController new];
        MSIMEFocusedController = previous;
        [previous fetchAccountGlosses:@[@"synthetic-a"] primary:@"en" secondary:@"" generation:1];
        MSIMEFocusedController = current;
        [current fetchAccountGlosses:@[@"synthetic-b"] primary:@"en" secondary:@"" generation:2];
        [previous cancelQueuedAccountGlosses];
        assert(fixturePending() == 2 && "迟到的旧控制器不能取消新控制器的等待页");
        [bystander cancelQueuedAccountGlosses];
        assert(fixturePending() == 2 && "没有发出请求的控制器不能取消等待页");
        [current cancelQueuedAccountGlosses];

        assert(fixturePending() == 0 && "所有者关闭翻译时仍须取消自己的等待页");
        MSIMEFocusedController = previous;
        [previous fetchAccountGlosses:@[@"synthetic-c"] primary:@"en" secondary:@"" generation:3];
        [previous fetchAccountGlosses:@[@"synthetic-d"] primary:@"en" secondary:@"" generation:4];
        assert(fixturePending() == 4 && "同一控制器的新页替换自己的旧页");
        fixtureOnCancel(^{
            MSIMEFocusedController = current;
            [current fetchAccountGlosses:@[@"synthetic-e"] primary:@"en" secondary:@"" generation:5];
        });
        [previous cancelQueuedAccountGlosses];
        [previous cancelQueuedAccountGlosses];
        assert(fixturePending() == 5 && "取消回调中发出的新请求不能被旧所有者再次取消");
        [current cancelQueuedAccountGlosses];
        assert(fixturePending() == 0 && "接管后的所有者仍能取消自己的等待页");
        MSIMEFocusedController = previous;
        [previous fetchAccountGlosses:@[@"synthetic-f"] primary:@"en" secondary:@"" generation:6];
        MSIMEFocusedController = current;
        [current cancelQueuedAccountGlosses];
        assert(fixturePending() == 0 && "当前焦点退出账号翻译时须清理旧等待页");
        [current fetchAccountGlosses:@[@"synthetic-g"] primary:@"en" secondary:@"" generation:7];
        [previous fetchAccountGlosses:@[@"synthetic-h"] primary:@"en" secondary:@"" generation:8];
        assert(fixturePending() == 7 && "旧计时器不能在失焦后替换当前控制器的等待页");
        [current cancelQueuedAccountGlosses];

        [current fetchOnDeviceGlosses:@[@"synthetic-current"] targets:@[@"en", @"ja"]];
        assert(([fixtureDeviceWords() isEqual:@[@"synthetic-current"]]
               && [fixtureDeviceTargets() isEqual:@[@"en", @"ja"]]
               && "焦点控制器的系统翻译页和目标语言须完整交给后端"));
        [previous fetchOnDeviceGlosses:@[@"synthetic-stale"] targets:@[@"en"]];
        assert([fixtureDeviceWords() isEqual:@[@"synthetic-current"]]
               && "旧词典回调不能替换当前控制器的系统翻译等待页");
        MSIMEFocusedController = nil;
        [current fetchOnDeviceGlosses:@[@"synthetic-unfocused"] targets:@[@"en"]];
        assert([fixtureDeviceWords() isEqual:@[@"synthetic-current"]]
               && "失焦后的系统翻译回调不能提交新页");
        MSIMEFocusedController = previous;
        [previous fetchOnDeviceGlosses:@[@"synthetic-refocused"] targets:@[@"en"]];
        assert([fixtureDeviceWords() isEqual:@[@"synthetic-refocused"]]
               && "重新取得焦点的控制器仍能替换系统翻译等待页");
    }
}
'''

backend = '''#import <Foundation/Foundation.h>
#include <cstdint>
static uint64_t pending;
static void (^onCancel)(void);
static NSArray *deviceWords;
static NSArray *deviceTargets;
extern "C" void MSIMEFetchOnDeviceCandidateGlosses(const char *words, const char *targets) {
    deviceWords = [NSJSONSerialization JSONObjectWithData:[@(words) dataUsingEncoding:NSUTF8StringEncoding]
                                                 options:0 error:nil];
    deviceTargets = [NSJSONSerialization JSONObjectWithData:[@(targets) dataUsingEncoding:NSUTF8StringEncoding]
                                                   options:0 error:nil];
}
extern "C" NSArray *fixtureDeviceWords(void) { return deviceWords; }
extern "C" NSArray *fixtureDeviceTargets(void) { return deviceTargets; }
extern "C" void MSIMEFetchAccountCandidateGlosses(const char *, const char *, const char *, uint64_t generation) {
    pending = generation;
}
extern "C" void MSIMECancelAccountCandidateGlosses(void) {
    pending = 0;
    void (^callback)(void) = onCancel;
    onCancel = nil;
    if (callback) callback();
}
extern "C" uint64_t fixturePending(void) { return pending; }
extern "C" void fixtureOnCancel(void (^callback)(void)) { onCancel = [callback copy]; }
'''

with tempfile.TemporaryDirectory(prefix="msime-account-gloss-owner-") as directory:
    root = Path(directory)
    source = root / "controller.mm"
    stub = root / "backend.mm"
    executable = root / "owner-test"
    source.write_text(program, encoding="utf-8")
    stub.write_text(backend, encoding="utf-8")
    subprocess.run(
        [compiler, "-std=c++17", "-fobjc-arc", "-Wall", "-Wextra", "-Werror", "-UNDEBUG",
         str(source), str(stub), "-framework", "Foundation", "-o", str(executable)],
        check=True, timeout=60,
    )
    subprocess.run([str(executable)], check=True, timeout=10)

print("macOS 候选释义队列：实际控制器方法的八项账号、四项系统翻译归属回归通过")
