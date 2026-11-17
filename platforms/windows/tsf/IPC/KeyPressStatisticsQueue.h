#pragma once
#include "../../common/TypingSource.h"

// Count one physical key press for the key heatmap. `keyId` is a canonical id from KeyPressStatistics.h. Never blocks the caller: the pipe work happens on the thread pool, and nothing is buffered until the Server has answered that statistics are on.
void QueueKeyPressStatistics(const wchar_t *keyId, msime::windows::TypingSource source = msime::windows::TypingSource::Unknown);

// Hand whatever has been counted to the Server now, for focus loss and deactivation; the size and timer triggers flush on their own.
void FlushKeyPressStatistics();
