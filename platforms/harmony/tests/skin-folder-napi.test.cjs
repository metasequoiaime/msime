// 执行实际 NAPI 工作对象与排队代码；只替换 NAPI 调度，文件和 fd 使用合成目录。
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const repo = path.resolve(__dirname, '../../..');
const native = path.join(repo, 'platforms/harmony/native');
const source = fs.readFileSync(path.join(native, 'client_napi.cpp'), 'utf8');
const begin = source.indexOf('struct SkinFolderWork {');
const end = source.indexOf('static napi_value ResourcePackAdopt(', begin);
assert.ok(begin >= 0 && end > begin, '找不到皮肤目录 NAPI 工作代码');
const actual = source.slice(begin, end);
const prefix = String.raw`
#include "skin_folder_copy.h"
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <cerrno>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>
using napi_env = void *; using napi_callback_info = void *; using napi_value = void *;
using napi_async_work = void *; using napi_deferred = void *; using napi_status = int;
constexpr int napi_ok=0; constexpr size_t NAPI_AUTO_LENGTH=static_cast<size_t>(-1);
static int inputFd=-1, outputFd=-1, failure=0, invalidCalls=0, resolved=0, rejected=0, deleted=0;
static void *captured=nullptr, *resolvedValue=nullptr;
static bool arguments(napi_env, napi_callback_info, size_t n, std::vector<napi_value> &out) {
    if (n!=2) { return false; }
    out={reinterpret_cast<void *>(static_cast<intptr_t>(inputFd)),
      reinterpret_cast<void *>(static_cast<intptr_t>(outputFd))}; return true;
}
static bool argumentInt32(napi_env, napi_value value, int32_t &out) {out=static_cast<int32_t>(reinterpret_cast<intptr_t>(value)); return true;}
static napi_value invalid(napi_env, const char *) {++invalidCalls; return nullptr;}
static void rejectWith(napi_env, napi_deferred, const char *) {++rejected;}
static int napi_create_promise(napi_env,napi_deferred *deferred,napi_value *promise) {
    if(failure==1) { return 1; }
    *deferred=reinterpret_cast<void *>(1); *promise=reinterpret_cast<void *>(2); return napi_ok;
}
static int napi_create_string_utf8(napi_env,const char *,size_t,napi_value *value) {
    if(failure==2) { return 1; }
    *value=reinterpret_cast<void *>(3); return napi_ok;
}
static int napi_create_async_work(napi_env,napi_value,napi_value,
  void (*)(napi_env,void *),void (*)(napi_env,napi_status,void *),void *data,napi_async_work *work) {
    if(failure==3) { return 1; }
    captured=data; *work=reinterpret_cast<void *>(4); return napi_ok;
}
static int napi_queue_async_work(napi_env,napi_async_work) {return failure==4?1:napi_ok;}
static int napi_delete_async_work(napi_env,napi_async_work) {++deleted;return napi_ok;}
static int napi_get_boolean(napi_env,bool value,napi_value *out) {
    if(failure==5) { return 1; }
    *out=reinterpret_cast<void *>(static_cast<intptr_t>(value?6:7));return napi_ok;
}
static int napi_resolve_deferred(napi_env,napi_deferred,napi_value value) {++resolved;resolvedValue=value;return napi_ok;}
`;
const suffix = String.raw`
static void must(bool ok,const char *msg) {if(!ok){fprintf(stderr,"FAIL %s errno=%d\n",msg,errno);exit(1);}}
static int fdcount() {int count=0;for(int fd=0;fd<1024;fd++) if(fcntl(fd,F_GETFD)!=-1) ++count;return count;}
int main(int argc,char **argv) {
#ifdef __OHOS__
    fdsan_set_error_level(FDSAN_ERROR_LEVEL_FATAL);
#endif
    must(argc==2,"fixture path");std::string root=argv[1];
    must(mkdir((root+"/source").c_str(),0700)==0 && mkdir((root+"/target").c_str(),0700)==0,"fixture directories");
    int file=open((root+"/source/skin.toml").c_str(),O_WRONLY|O_CREAT|O_EXCL,0600);
    must(file>=0 && write(file,"data",4)==4 && close(file)==0,"fixture file");
    inputFd=open((root+"/source").c_str(),O_RDONLY|O_DIRECTORY);
    outputFd=open((root+"/target").c_str(),O_RDONLY|O_DIRECTORY);
    must(inputFd>=0 && outputFd>=0,"fixture fds");
    const int baseline=fdcount();
    for(failure=1;failure<=4;failure++) {
        captured=nullptr; int beforeInvalid=invalidCalls;
        must(StageSkinFolder(nullptr,nullptr)==nullptr && invalidCalls==beforeInvalid+1,"queue error reported");
        must(fdcount()==baseline,"queue failure closes both duplicates");
    }
    failure=0;
    int savedInput=inputFd,savedOutput=outputFd;
    outputFd=100000;
    must(StageSkinFolder(nullptr,nullptr)==nullptr && fdcount()==baseline,"invalid destination closes source duplicate");
    outputFd=savedOutput; inputFd=100000;
    must(StageSkinFolder(nullptr,nullptr)==nullptr && fdcount()==baseline,"invalid source closes destination duplicate");
    inputFd=savedInput; captured=nullptr;
    must(StageSkinFolder(nullptr,nullptr)!=nullptr && captured,"queue worker");
    auto *work=static_cast<SkinFolderWork *>(captured);int src=work->source,dst=work->destination;
    must(src!=inputFd && dst!=outputFd && fcntl(src,F_GETFD)>=0 && fcntl(dst,F_GETFD)>=0,"independent fds");
    must(close(inputFd)==0 && close(outputFd)==0,"original owner closes descriptors");inputFd=outputFd=-1;
    executeSkinFolder(nullptr,work); must(work->copied,"worker uses retained fds");
    completeSkinFolder(nullptr,napi_ok,work);
    must(resolved==1 && fcntl(src,F_GETFD)==-1 && fcntl(dst,F_GETFD)==-1,"completion closes both duplicates");
    int target=open((root+"/target/skin.toml").c_str(),O_RDONLY); char buffer[5]={};
    must(target>=0 && read(target,buffer,4)==4 && std::string(buffer)=="data" && close(target)==0,"copied bytes");
    inputFd=open((root+"/source").c_str(),O_RDONLY|O_DIRECTORY);
    outputFd=open((root+"/target").c_str(),O_RDONLY|O_DIRECTORY);
    const int nextBaseline=fdcount();captured=nullptr;
    must(StageSkinFolder(nullptr,nullptr)!=nullptr && captured,"queue copy failure");
    auto *failedCopy=static_cast<SkinFolderWork *>(captured);
    executeSkinFolder(nullptr,failedCopy);must(!failedCopy->copied,"duplicate output refused");
    completeSkinFolder(nullptr,napi_ok,failedCopy);
    must(resolved==2 && resolvedValue==reinterpret_cast<void *>(7) && fdcount()==nextBaseline,
      "copy failure returns false and closes both duplicates");
    captured=nullptr;
    must(StageSkinFolder(nullptr,nullptr)!=nullptr && captured,"queue cancellation");
    completeSkinFolder(nullptr,1,captured);
    must(rejected==1 && fdcount()==nextBaseline,"cancel closes both duplicates");
    failure=5;captured=nullptr;
    must(StageSkinFolder(nullptr,nullptr)!=nullptr && captured,"queue boolean error");
    completeSkinFolder(nullptr,napi_ok,captured);
    must(rejected==2 && fdcount()==nextBaseline,"conversion error closes both duplicates");
    must(close(inputFd)==0 && close(outputFd)==0,"caller closes originals");
    fprintf(stdout,"NAPI skin worker: queue errors, cancel and fd ownership passed\n");
}
`;
const out = path.join(repo, 'target/harmony-tests/skin-folder-napi');
fs.mkdirSync(out, {recursive:true});
const cpp = path.join(out, 'adapter.cpp');
const binary = path.join(out, 'adapter');
fs.writeFileSync(cpp, prefix + actual + suffix);
execFileSync(process.env.CXX || 'c++', ['-std=c++17','-O0','-Wall','-Wextra','-Werror',
  '-I' + native, cpp, path.join(native, 'skin_folder_copy.cpp'), '-o', binary], {stdio:'inherit'});
const scratch = fs.mkdtempSync(path.join(out, 'fixture-'));
try {execFileSync(binary, [scratch], {stdio:'inherit'});} finally {fs.rmSync(scratch, {recursive:true,force:true});}
