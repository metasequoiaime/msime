#import "../../src/voice/VoiceProviderSocket.h"
#include <cassert>
#include <thread>
#include <set>
#include <vector>
uint64_t VoiceSocketLeaseFromOtherEntry();
int main() {
    const uint64_t local = MSIMENextVoiceSocketLease();
    const uint64_t global = VoiceSocketLeaseFromOtherEntry();
    assert(local && global > local && MSIMENextVoiceSocketLease() > global);
    // 不同编译单元与并发入口必须共用同一个进程内身份空间。
    std::vector<uint64_t> entries[8];
    std::vector<std::thread> workers;
    for (int i = 0; i < 8; ++i) workers.emplace_back([&, i] {
        for (int n = 0; n < 128; ++n) entries[i].push_back(i % 2 ? VoiceSocketLeaseFromOtherEntry() : MSIMENextVoiceSocketLease());
    });
    for (auto &worker : workers) worker.join();
    std::set<uint64_t> identities;
    for (auto &entry : entries) for (auto value : entry) assert(value > global && identities.insert(value).second);
    assert(identities.size() == 1024);
}
