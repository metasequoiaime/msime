// build.sh 通过 -include 把本头文件强制包含进鸿蒙版 ONNX Runtime 的每个 C++ 文件，其他平台不会包含它。
//
// 鸿蒙的 musl 提供 sched_setaffinity，却没有 pthread_setaffinity_np。ONNX Runtime 的 posix/env.cc 总是在新线程自己身上调用后者，参数是 pthread_self()，只有 Android、Apple、wasm 和 AIX 被排除。对调用线程而言，两者作用相同：sched_setaffinity 的 pid 为 0 时设置调用线程的亲和性。pthread_setaffinity_np 通过返回值而非 errno 报告失败，因此这里也返回相同的错误码。只有会话要求线程亲和性时才会执行这段代码，逐键重排器没有这个要求。
//
// 使用宏而不是函数，因此本头文件自身不需要包含其他头文件：宏在 env.cc 的调用处展开，那时 env.cc 已经在定义 _GNU_SOURCE 后包含了 <sched.h> 和 <cerrno>。若在这里抢先包含 <sched.h>，就会赶在各文件定义 _GNU_SOURCE 之前，导致它们看不到 cpu_set_t。
#pragma once

#define pthread_setaffinity_np(thread, size, set) ((void)(thread), sched_setaffinity(0, (size), (set)) == 0 ? 0 : errno)
