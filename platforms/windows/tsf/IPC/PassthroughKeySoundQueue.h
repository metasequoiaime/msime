#pragma once

#include <cstdint>

// 把一次交给应用的按下排给 Server，让它出按键音、计入打字特效的连击（Aux 管道的 KeySound）。不阻塞调用方，按键路径上只有一次入队：管道在线程池上写。Server 没有回 "OK"（按键音和打字特效都关着，或者 Server 不在）时停发 10 秒。
void QueuePassthroughKeySound(uint64_t clientId, uint64_t focusToken, uint32_t keyClass);
// 正在停发：Server 上次没有回 "OK"，10 秒还没过。按键路径先问这一句，功能关着时就不必再读隔间、查键盘状态。
bool PassthroughKeySoundSuppressed();
