// 合成进程用真实内存故障验证业务首帧，不读取输入或真实应用状态。
#include <assert.h>
#include <dlfcn.h>
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
  assert(argc == 2 || argc == 3);
  void (*fault)(void *) = synthetic_fault;
  if (argc == 3) {
    void *library = dlopen(argv[2], RTLD_NOW | RTLD_LOCAL);
    assert(library != NULL);
    fault = (void (*)(void *))dlsym(library, "synthetic_library_fault");
    assert(fault != NULL);
  }
  void *page = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
  assert(page != MAP_FAILED);
  const pid_t pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    msime_ios_crash_handlers_install();
    msime_ios_crash_record_set(argv[1], "MSIME-CRASH/1 {}\n");
    fault(page);
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
  Dl_info image;
  assert(dladdr((const void *)fault, &image) != 0);
  const uintptr_t function = (uintptr_t)fault - (uintptr_t)image.dli_fbase;
  assert(pc >= function && pc < function + 128);
  assert(strstr(line, " [id 0x") != NULL);
  assert(strchr(line, '/') == NULL);
  const char *module = argc == 3 ? argv[2] : argv[0];
  const char *name = strrchr(module, '/');
  name = name == NULL ? module : name + 1;
  assert(strncmp(line, name, strlen(name)) == 0 && line[strlen(name)] == '+');
  return 0;
}
