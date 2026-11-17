#pragma once

// stdin 接受带选区代次的写入；stdout 交付完整选区状态和写入回执，不创建窗口。
int msime_run_wayland_clipboard_bridge();
