#include "PassthroughKeySoundQueue.h"

#include "Ipc.h"
#include "../../common/AuxMessage.h"

#include <array>
#include <atomic>
#include <windows.h>

namespace
{
struct QueuedKey
{
    msime::windows::AuxKeySound key;
    ULONGLONG queuedAt = 0;
};

// 定长环形队列：按键路径上入队不分配内存。Server 卡住时最多攒这么多，再多的键直接丢，晚到的按键音比没有更糟。
constexpr std::size_t MaximumPendingKeys = 8;
// 排得比这更久的键不再发：那一下早过去了。
constexpr ULONGLONG StaleMilliseconds = 250;
// 没有 "OK" 之后停发这么久。也是在设置里打开按键音之后，交给应用的键最晚多久开始出声。
constexpr ULONGLONG SuppressionMilliseconds = 10000;

SRWLOCK g_queueLock = SRWLOCK_INIT;
std::array<QueuedKey, MaximumPendingKeys> g_pending{};
std::size_t g_head = 0;
std::size_t g_count = 0;
bool g_flushInFlight = false;
std::atomic<ULONGLONG> g_suppressedUntil{0};

void CALLBACK FlushPassthroughKeySounds(PTP_CALLBACK_INSTANCE instance, void *context)
{
    // 提交时拿的模块引用在回调返回之后才释放，DLL 在回调运行期间不会被卸载。
    FreeLibraryWhenCallbackReturns(instance, static_cast<HMODULE>(context));
    for (;;)
    {
        QueuedKey next{};
        AcquireSRWLockExclusive(&g_queueLock);
        if (g_count == 0)
        {
            // 在锁里清掉，之后入队的键会再提交一次。
            g_flushInFlight = false;
            ReleaseSRWLockExclusive(&g_queueLock);
            break;
        }
        next = g_pending[g_head];
        g_head = (g_head + 1) % MaximumPendingKeys;
        --g_count;
        ReleaseSRWLockExclusive(&g_queueLock);
        if (GetTickCount64() - next.queuedAt > StaleMilliseconds)
        {
            continue;
        }
        if (!SendToAuxNamedpipe(msime::windows::aux_key_sound_message(next.key), true))
        {
            g_suppressedUntil.store(GetTickCount64() + SuppressionMilliseconds, std::memory_order_relaxed);
            AcquireSRWLockExclusive(&g_queueLock);
            g_head = 0;
            g_count = 0;
            g_flushInFlight = false;
            ReleaseSRWLockExclusive(&g_queueLock);
            break;
        }
    }
}
} // namespace

bool PassthroughKeySoundSuppressed()
{
    return GetTickCount64() < g_suppressedUntil.load(std::memory_order_relaxed);
}

void QueuePassthroughKeySound(uint64_t clientId, uint64_t focusToken, uint32_t keyClass)
{
    if (clientId == 0 || focusToken == 0 || PassthroughKeySoundSuppressed())
    {
        return;
    }
    AcquireSRWLockExclusive(&g_queueLock);
    if (g_count < MaximumPendingKeys)
    {
        g_pending[(g_head + g_count) % MaximumPendingKeys] = QueuedKey{{clientId, focusToken, keyClass}, GetTickCount64()};
        ++g_count;
    }
    if (!g_flushInFlight)
    {
        // 和直通统计一样：拿一个加载器引用（不是 DllCanUnloadNow 读的 COM 计数）让 DLL 在回调运行时保持映射。
        HMODULE module = nullptr;
        if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                               reinterpret_cast<LPCWSTR>(&FlushPassthroughKeySounds), &module))
        {
            g_flushInFlight = true;
            if (!TrySubmitThreadpoolCallback(FlushPassthroughKeySounds, module, nullptr))
            {
                g_flushInFlight = false;
                FreeLibrary(module);
            }
        }
    }
    ReleaseSRWLockExclusive(&g_queueLock);
}
