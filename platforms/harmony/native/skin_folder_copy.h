#pragma once

// 两个 fd 均由调用方拥有；源和目标必须是已打开的目录。
// 失败可能留下不完整目标，调用方负责移除暂存树。
bool copySkinFolderByFd(int source, int destination);
