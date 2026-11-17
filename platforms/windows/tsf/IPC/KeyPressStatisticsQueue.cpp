#include "KeyPressStatisticsQueue.h"

#include "Ipc.h"
#include "../../common/AuxTypingKeySources.h"
#include "../../common/TypingKeyBatch.h"

#include <cstdint>
#include <map>
#include <string>
#include <utility>
#include <vector>
#include <windows.h>
#include "fmt/xchar.h"

namespace
{
// 日期在按下时固定，封批和发送不会重标记历史来源。
using KeyBatch = msime::windows::TypingKeyBatch;

// Statistics ship disabled and the TIP cannot read the switch itself, so the Server's answer is the switch: nothing is counted until a probe has been answered "OK", and the first unanswered batch drops everything and stops counting again.
enum class Consent
{
    Unknown,
    Probing,
    Enabled,
    Refused,
};

SRWLOCK g_queueLock = SRWLOCK_INIT;
KeyBatch g_pending;
unsigned g_pendingPresses = 0;
std::vector<KeyBatch> g_outbox;
bool g_drainInFlight = false;
bool g_probeRequested = false;
bool g_timerArmed = false;
Consent g_consent = Consent::Unknown;
bool g_sourceFormat = false;
ULONGLONG g_refusedUntil = 0;

constexpr unsigned FlushPresses = 256;
constexpr ULONGLONG FlushIntervalMilliseconds = 30000;
// How long a refusal is believed. It is also how soon turning statistics on starts counting keys in an application that was already running.
constexpr ULONGLONG RefusalMilliseconds = 30000;
// Bounds what a stalled Server can make this process hold; a batch past it is a dropped count.
constexpr std::size_t MaximumOutboxBatches = 8;

std::wstring LocalDay()
{
    SYSTEMTIME local = {};
    GetLocalTime(&local);
    return fmt::format(L"{:04}-{:02}-{:02}", local.wYear, local.wMonth, local.wDay);
}

void SealPendingLocked()
{
    if (g_pending.counts.empty())
    {
        return;
    }
    if (g_outbox.size() < MaximumOutboxBatches)
    {
        g_outbox.push_back(std::move(g_pending));
    }
    g_pending = {};
    g_pendingPresses = 0;
}

bool SendBatch(const KeyBatch &batch, bool sourceFormat)
{
    if (sourceFormat)
    {
        for (const auto &[source, counts] : batch.sources)
            for (const auto &message : msime::windows::aux_typing_key_sources_messages(batch.day, source, counts))
                if (!SendToAuxNamedpipe(message, true)) return false;
        return true;
    }
    for (const auto &message : msime::windows::aux_typing_keys_messages(batch.day, batch.counts))
    {
        if (!SendToAuxNamedpipe(message, true))
        {
            return false;
        }
    }
    return true;
}

void CALLBACK DrainKeyPressStatistics(PTP_CALLBACK_INSTANCE instance, void *context)
{
    // Drops the submit-time loader reference only after this callback has returned.
    FreeLibraryWhenCallbackReturns(instance, static_cast<HMODULE>(context));
    for (;;)
    {
        std::vector<KeyBatch> batches;
        AcquireSRWLockExclusive(&g_queueLock);
        batches.swap(g_outbox);
        const bool probe = g_probeRequested;
        bool sourceFormat = g_sourceFormat;
        g_probeRequested = false;
        if (batches.empty() && !probe)
        {
            // Cleared under the lock so a batch sealed after this point submits a new drain.
            g_drainInFlight = false;
            ReleaseSRWLockExclusive(&g_queueLock);
            break;
        }
        ReleaseSRWLockExclusive(&g_queueLock);

        bool accepted = true;
        if (probe)
        {
            const auto format = msime::windows::probe_typing_key_sources(LocalDay(), [](const std::wstring &message) {
                return SendToAuxNamedpipe(message, true);
            });
            accepted = format.has_value();
            if (format) sourceFormat = *format;
        }
        for (const auto &batch : batches)
        {
            if (!accepted)
            {
                break;
            }
            accepted = SendBatch(batch, sourceFormat);
        }

        AcquireSRWLockExclusive(&g_queueLock);
        if (accepted)
        {
            g_consent = Consent::Enabled;
            g_sourceFormat = sourceFormat;
        }
        else
        {
            // Statistics are off or nobody is listening: whatever was counted since the last answer is dropped rather than held for a switch that may never turn on.
            g_consent = Consent::Refused;
            g_refusedUntil = GetTickCount64() + RefusalMilliseconds;
            g_pending = {};
            g_pendingPresses = 0;
            g_outbox.clear();
        }
        ReleaseSRWLockExclusive(&g_queueLock);
    }
}

// True when a drain is running or has been submitted, so whatever is queued now will be sent.
bool SubmitDrainLocked()
{
    if (g_drainInFlight)
    {
        return true;
    }
    // A loader reference (not the COM lock count, which DllCanUnloadNow reads) keeps the DLL mapped while the callback runs.
    HMODULE module = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, reinterpret_cast<LPCWSTR>(&DrainKeyPressStatistics),
                            &module))
    {
        return false;
    }
    g_drainInFlight = true;
    if (!TrySubmitThreadpoolCallback(DrainKeyPressStatistics, module, nullptr))
    {
        g_drainInFlight = false;
        FreeLibrary(module);
        return false;
    }
    return true;
}

void CALLBACK FlushKeyPressStatisticsTimer(PTP_CALLBACK_INSTANCE instance, void *context, PTP_TIMER timer)
{
    // The timer is one-shot and owned by this callback: closing it here frees it once the callback returns, and the loader reference taken when it was armed goes with it.
    FreeLibraryWhenCallbackReturns(instance, static_cast<HMODULE>(context));
    CloseThreadpoolTimer(timer);
    AcquireSRWLockExclusive(&g_queueLock);
    g_timerArmed = false;
    SealPendingLocked();
    if (!g_outbox.empty())
    {
        SubmitDrainLocked();
    }
    ReleaseSRWLockExclusive(&g_queueLock);
}

void ArmTimerLocked()
{
    if (g_timerArmed)
    {
        return;
    }
    HMODULE module = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                            reinterpret_cast<LPCWSTR>(&FlushKeyPressStatisticsTimer), &module))
    {
        return;
    }
    PTP_TIMER timer = CreateThreadpoolTimer(FlushKeyPressStatisticsTimer, module, nullptr);
    if (timer == nullptr)
    {
        FreeLibrary(module);
        return;
    }
    // A negative due time is relative, in 100-nanosecond units.
    const LONGLONG due = -static_cast<LONGLONG>(FlushIntervalMilliseconds) * 10000;
    FILETIME dueTime = {};
    dueTime.dwLowDateTime = static_cast<DWORD>(static_cast<ULONGLONG>(due) & 0xFFFFFFFFu);
    dueTime.dwHighDateTime = static_cast<DWORD>(static_cast<ULONGLONG>(due) >> 32);
    SetThreadpoolTimer(timer, &dueTime, 0, 0);
    g_timerArmed = true;
}
} // namespace

void QueueKeyPressStatistics(const wchar_t *keyId, msime::windows::TypingSource source)
{
    const ULONGLONG now = GetTickCount64();
    std::wstring day = LocalDay();
    AcquireSRWLockExclusive(&g_queueLock);
    if (g_consent != Consent::Enabled)
    {
        // Unknown, or a refusal that has run out: ask before counting anything. While the answer is pending, or a refusal still holds, the press is not counted at all.
        if (g_consent == Consent::Unknown || (g_consent == Consent::Refused && now >= g_refusedUntil))
        {
            g_probeRequested = true;
            // A probe that could not be submitted leaves the state as it was, so the next press asks again.
            if (SubmitDrainLocked())
            {
                g_consent = Consent::Probing;
            }
            else
            {
                g_probeRequested = false;
            }
        }
        ReleaseSRWLockExclusive(&g_queueLock);
        return;
    }
    if (!g_pending.counts.empty() && g_pending.day != day)
    {
        // Midnight passed: the old day leaves first, under its own date.
        SealPendingLocked();
        SubmitDrainLocked();
    }
    if (g_pending.counts.empty())
    {
        g_pending.day = std::move(day);
        ArmTimerLocked();
    }
    g_pending.add(keyId, source);
    if (++g_pendingPresses >= FlushPresses)
    {
        SealPendingLocked();
        SubmitDrainLocked();
    }
    ReleaseSRWLockExclusive(&g_queueLock);
}

void FlushKeyPressStatistics()
{
    AcquireSRWLockExclusive(&g_queueLock);
    SealPendingLocked();
    if (!g_outbox.empty())
    {
        SubmitDrainLocked();
    }
    ReleaseSRWLockExclusive(&g_queueLock);
}
