// 测试使用合成目录；直接包含实现以注入精确的并发替换时机。
#define MSIME_SKIN_COPY_TESTING
#include "../native/skin_folder_copy.cpp"
#include <cstdio>
#include <cstdlib>
#include <string>
static void must(bool b,const char *label){if(!b){fprintf(stderr,"FAIL %s errno=%d\n",label,errno);exit(1);}}
static void put(const std::string &p,const char *text) {Fd f(open(p.c_str(),O_WRONLY|O_CREAT|O_EXCL,0600));must(f.value>=0,"create fixture");must(write(f.value,text,strlen(text))==(ssize_t)strlen(text),"write fixture");}
int main(int argc,char **argv) {
#ifdef __OHOS__
  // 仅测试进程启用：描述符被错误关闭时立即失败。
  fdsan_set_error_level(FDSAN_ERROR_LEVEL_FATAL);
#endif
  must(argc==2,"scratch arg");std::string r=argv[1];
  must(singleName("skin.toml") && !singleName("") && !singleName("..")
    && !singleName("nested/link"), "only one path segment may reach openat");
  for(auto s:{"/src","/out","/src/sub","/race","/raceout","/race/sub","/outside","/linksrc","/linkout","/fileout","/shortout","/hardout","/grow","/growout","/dupout","/publicout","/deep","/deepout","/fifo","/fifoout","/filerace","/fileraceout"})must(mkdir((r+s).c_str(),0700)==0,"mkdir");
  put(r+"/src/skin.toml","fixture");put(r+"/src/sub/image","data");put(r+"/race/sub/good","safe");put(r+"/outside/private","private");
  Fd src(open((r+"/src").c_str(),O_RDONLY|O_DIRECTORY)),out(open((r+"/out").c_str(),O_RDONLY|O_DIRECTORY)); Budget one; must(tree(src.value,out.value,0,one,[](int,const char*){}),"valid tree");
  Fd image(open((r+"/out/sub/image").c_str(),O_RDONLY)); char data[5]={};must(image.value>=0&&read(image.value,data,4)==4&&!strcmp(data,"data"),"bytes");
  Fd publicout(open((r+"/publicout").c_str(),O_RDONLY|O_DIRECTORY));
  must(copySkinFolderByFd(src.value,publicout.value),"public native entry");
  int original=open((r+"/src").c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC);must(original>=0,"open owner");
  Fd duplicate(fcntl(original,F_DUPFD_CLOEXEC,0));must(duplicate.value>=0,"duplicate for worker");
  must(close(original)==0,"close owner"); Fd dupout(open((r+"/dupout").c_str(),O_RDONLY|O_DIRECTORY));
  Budget owned; must(tree(duplicate.value,dupout.value,0,owned,[](int,const char*){}),"worker retains directory fd");
  Fd race(open((r+"/race").c_str(),O_RDONLY|O_DIRECTORY)), raceout(open((r+"/raceout").c_str(),O_RDONLY|O_DIRECTORY)); bool fired=false;
  Budget two; bool ok=tree(race.value,raceout.value,0,two,[&](int parent,const char *name){if(!fired&&!strcmp(name,"sub")){fired=true;must(renameat(parent,"sub",parent,"moved")==0,"move");must(symlinkat((r+"/outside").c_str(),parent,"sub")==0,"symlink");}});
  must(fired&&!ok,"directory swap refused");must(access((r+"/raceout/sub/private").c_str(),F_OK)!=0,"outside not copied");
  must(symlink((r+"/outside").c_str(),(r+"/linksrc/link").c_str())==0,"link setup");Fd links(open((r+"/linksrc").c_str(),O_RDONLY|O_DIRECTORY)),linkout(open((r+"/linkout").c_str(),O_RDONLY|O_DIRECTORY)); Budget three;must(!tree(links.value,linkout.value,0,three,[](int,const char*){}),"link refused");
  put(r+"/filerace/skin.toml","safe");
  Fd filerace(open((r+"/filerace").c_str(),O_RDONLY|O_DIRECTORY));
  Fd fileraceout(open((r+"/fileraceout").c_str(),O_RDONLY|O_DIRECTORY));
  bool fileSwapped=false; Budget fileBudget;
  bool fileOk=tree(filerace.value,fileraceout.value,0,fileBudget,[&](int parent,const char *name){
    if(!fileSwapped&&!strcmp(name,"skin.toml")) {
      fileSwapped=true;
      must(renameat(parent,"skin.toml",parent,"saved")==0,"move file");
      must(symlinkat((r+"/outside/private").c_str(),parent,"skin.toml")==0,"swap file for link");
    }
  });
  must(fileSwapped&&!fileOk && access((r+"/fileraceout/skin.toml").c_str(),F_OK)!=0,
    "file symlink swap refused");
  Fd fileout(open((r+"/fileout").c_str(),O_RDONLY|O_DIRECTORY)); Budget low; low.bytes=2;
  must(!tree(src.value,fileout.value,0,low,[](int,const char*){}),"byte budget");
  Fd shortout(open((r+"/shortout").c_str(),O_RDONLY|O_DIRECTORY)); Budget few; few.entries=0;
  must(!tree(src.value,shortout.value,0,few,[](int,const char*){}),"entry budget");
  std::string deepPath=r+"/deep";
  for(int i=0;i<17;i++){deepPath+="/x";must(mkdir(deepPath.c_str(),0700)==0,"deep fixture");}
  Fd deep(open((r+"/deep").c_str(),O_RDONLY|O_DIRECTORY)),deepout(open((r+"/deepout").c_str(),O_RDONLY|O_DIRECTORY)); Budget five;
  must(!tree(deep.value,deepout.value,0,five,[](int,const char*){}),"depth budget");
  if(mkfifo((r+"/fifo/special").c_str(),0600)==0) {
    Fd fifos(open((r+"/fifo").c_str(),O_RDONLY|O_DIRECTORY)),fifoout(open((r+"/fifoout").c_str(),O_RDONLY|O_DIRECTORY));
    Budget fifoBudget;must(!tree(fifos.value,fifoout.value,0,fifoBudget,[](int,const char*){}),"FIFO refused");
    fprintf(stdout,"FIFO refused\n");
  } else {fprintf(stdout,"FIFO test skipped: fixture filesystem forbids FIFO (errno=%d)\n",errno);}
  put(r+"/grow/skin.toml","tiny");
  Fd grow(open((r+"/grow").c_str(),O_RDONLY|O_DIRECTORY)),growout(open((r+"/growout").c_str(),O_RDONLY|O_DIRECTORY));
  bool grew=false; afterOpened=[&](int parent,const char *name){if(!grew&&!strcmp(name,"skin.toml")){
    Fd append(openat(parent,name,O_WRONLY|O_APPEND|O_NOFOLLOW));must(append.value>=0&&write(append.value,"!",1)==1,"grow file");grew=true;}};
  Budget growBudget; must(!tree(grow.value,growout.value,0,growBudget,[](int,const char*){})&&grew,"file growth refused");
  Fd partial(open((r+"/growout/skin.toml").c_str(),O_RDONLY));struct stat partialStat{};
  must(partial.value>=0&&!fstat(partial.value,&partialStat)&&partialStat.st_size==4,"growth not copied beyond budget");
  afterOpened=[](int,const char*){};
  if(link((r+"/src/skin.toml").c_str(),(r+"/src/hardlink").c_str())==0) {
    Fd hardout(open((r+"/hardout").c_str(),O_RDONLY|O_DIRECTORY)); Budget four;
    must(!tree(src.value,hardout.value,0,four,[](int,const char*){}),"hard link refused");
    fprintf(stdout,"hard link refused\n");
  } else { fprintf(stdout,"hard link test skipped: fixture filesystem disallows hard links (errno=%d)\n",errno); }
  fprintf(stdout,"pinned openat probe: nested copy, duplicated fd, symlink swap, file growth and depth/size/count budgets passed\n");
}
