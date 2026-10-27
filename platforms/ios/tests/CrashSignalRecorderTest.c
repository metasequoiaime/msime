// 合成进程用真实内存故障验证业务首帧，不读取输入或真实应用状态。
#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

void msime_ios_crash_record_set(const char *, const char *);
void msime_ios_crash_handlers_install(void);

__attribute__((noinline)) void synthetic_fault(void *page) {
  *(volatile char *)page = 7;
}

int main(int argc, char **argv) {
  assert(argc == 2);
  void *page = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
  assert(page != MAP_FAILED);
  const pid_t pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    msime_ios_crash_handlers_install();
    msime_ios_crash_record_set(argv[1], "MSIME-CRASH/1 {}\n");
    synthetic_fault(page);
    _exit(0);
  }
  int status = 0;
  assert(waitpid(pid, &status, 0) == pid);
  assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
  assert(munmap(page, 4096) == 0);
  FILE *record = fopen(argv[1], "r");
  assert(record != NULL);
  char line[2048];
  assert(fgets(line, sizeof(line), record));
  assert(fgets(line, sizeof(line), record));
  assert(fgets(line, sizeof(line), record));
  fclose(record);
  const char *address = strstr(line, "0x");
  assert(address != NULL);
  const uintptr_t pc = (uintptr_t)strtoull(address, NULL, 16);
  const uintptr_t function = (uintptr_t)&synthetic_fault;
  assert(pc >= function && pc < function + 128);
  return 0;
}
