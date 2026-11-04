// Force-included (-include) into every C++ file of the HarmonyOS builds of ONNX Runtime by build.sh; the other platforms do not see it.
//
// HarmonyOS's musl has sched_setaffinity but no pthread_setaffinity_np. ONNX Runtime's posix/env.cc calls the latter from the new thread itself, always on pthread_self(), and leaves it out only for Android, Apple, wasm and AIX. For the calling thread the two are the same call: Linux's sched_setaffinity applies to the calling thread when pid is 0. pthread_setaffinity_np reports failure through its return value rather than errno, so the error is returned the same way. The code runs only when a session asks for thread affinity, which the keystroke reranker does not.
//
// A macro rather than a function, so this header needs no includes of its own: it expands where env.cc calls it, after env.cc has included <sched.h> with _GNU_SOURCE and <cerrno>. Including <sched.h> here, ahead of every file's own _GNU_SOURCE, would hide cpu_set_t from them.
#pragma once

#define pthread_setaffinity_np(thread, size, set) ((void)(thread), sched_setaffinity(0, (size), (set)) == 0 ? 0 : errno)
