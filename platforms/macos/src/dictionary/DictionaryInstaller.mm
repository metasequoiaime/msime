#import "DictionaryInstaller.h"
#import <CommonCrypto/CommonDigest.h>
#import <sqlite3.h>
#include <cerrno>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include "../core/SystemPathAlias.h"
static NSString *const MSIMEInstallerError = @"app.msime.client.dictionary-installer";
static const uint64_t MSIMEMaxDictionaryBytes = 128ull * 1024ull * 1024ull;
static BOOL Fail(NSError **e, NSString *s) { if (e) *e=[NSError errorWithDomain:MSIMEInstallerError code:1 userInfo:@{NSLocalizedDescriptionKey:s}]; return NO; }
// 词典目录的任何一层（目录本身也算）都不能是符号链接，只有目标核对过的 `/var`、`/tmp` 系统别名可以在它上面经过一次。
static BOOL RejectSymlinkAncestors(NSURL *url) {
    return !msime::mac::StoragePathIsSafe(url.URLByStandardizingPath.path.fileSystemRepresentation, false);
}
static BOOL StageDictionary(NSURL *source, NSURL *temporary, unsigned char digest[CC_SHA256_DIGEST_LENGTH], NSError **error) {
    int sourceFD = open(source.fileSystemRepresentation, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    if (sourceFD < 0) return Fail(error, @"词典无法打开");
    struct stat sourceStat = {};
    if (fstat(sourceFD, &sourceStat) != 0 || !S_ISREG(sourceStat.st_mode) || sourceStat.st_size <= 0 ||
        static_cast<uint64_t>(sourceStat.st_size) > MSIMEMaxDictionaryBytes) {
        close(sourceFD);
        return Fail(error, @"词典大小或类型无效");
    }
    int outputFD = open(temporary.fileSystemRepresentation, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (outputFD < 0) {
        close(sourceFD);
        return Fail(error, @"词典暂存失败");
    }
    auto abort = [&](NSString *message) {
        close(outputFD);
        close(sourceFD);
        unlink(temporary.fileSystemRepresentation);
        return Fail(error, message);
    };
    CC_SHA256_CTX context;
    CC_SHA256_Init(&context);
    uint8_t buffer[64 * 1024];
    uint64_t total = 0;
    for (;;) {
        ssize_t count = read(sourceFD, buffer, sizeof(buffer));
        if (count == 0) break;
        if (count < 0) {
            if (errno == EINTR) continue;
            return abort(@"词典读取失败");
        }
        if (static_cast<uint64_t>(count) > MSIMEMaxDictionaryBytes - total) return abort(@"词典过大");
        ssize_t written = 0;
        while (written < count) {
            ssize_t chunk = write(outputFD, buffer + written, static_cast<size_t>(count - written));
            if (chunk < 0) {
                if (errno == EINTR) continue;
                return abort(@"词典暂存失败");
            }
            if (chunk == 0) return abort(@"词典暂存失败");
            written += chunk;
        }
        CC_SHA256_Update(&context, buffer, static_cast<CC_LONG>(count));
        total += static_cast<uint64_t>(count);
    }
    struct stat finalStat = {};
    if (fstat(sourceFD, &finalStat) != 0 || finalStat.st_size != static_cast<off_t>(total) ||
        total != static_cast<uint64_t>(sourceStat.st_size)) return abort(@"词典在读取时发生变化");
    if (close(outputFD) != 0) {
        outputFD = -1;
        close(sourceFD);
        unlink(temporary.fileSystemRepresentation);
        return Fail(error, @"词典暂存失败");
    }
    outputFD = -1;
    close(sourceFD);
    sourceFD = -1;
    CC_SHA256_Final(digest, &context);
    return YES;
}
BOOL MSIMEInstallDictionary(NSURL *source, NSURL *directory, NSString *expected, NSError **error) {
    if (!source.isFileURL || !directory.isFileURL || expected.length != CC_SHA256_DIGEST_LENGTH * 2) return Fail(error,@"词典参数无效");
    if (RejectSymlinkAncestors(directory)) return Fail(error,@"词典目录不可使用符号链接");
    NSFileManager *fm=NSFileManager.defaultManager; if (![fm createDirectoryAtURL:directory withIntermediateDirectories:YES attributes:nil error:error]) return NO; NSURL *temp=[directory URLByAppendingPathComponent:@".msime-pinyin.db.installing"]; [fm removeItemAtURL:temp error:nil]; unsigned char digest[CC_SHA256_DIGEST_LENGTH]; if (!StageDictionary(source, temp, digest, error)) return NO; NSMutableString *actual=[NSMutableString string]; for (NSUInteger i=0;i<sizeof(digest);i++) [actual appendFormat:@"%02x",digest[i]]; if (![actual isEqualToString:expected.lowercaseString]) { [fm removeItemAtURL:temp error:nil]; return Fail(error,@"词典指纹不匹配"); }
    sqlite3 *db=NULL; if (sqlite3_open_v2(temp.fileSystemRepresentation,&db,SQLITE_OPEN_READONLY,NULL)!=SQLITE_OK) { if (db) sqlite3_close(db); [fm removeItemAtURL:temp error:nil]; return Fail(error,@"词典无法打开"); } sqlite3_stmt *stmt=NULL; BOOL valid=sqlite3_prepare_v2(db,"PRAGMA quick_check(1)",-1,&stmt,NULL)==SQLITE_OK && sqlite3_step(stmt)==SQLITE_ROW && strcmp((const char *)sqlite3_column_text(stmt,0),"ok")==0; sqlite3_finalize(stmt); sqlite3_close(db); if (!valid) { [fm removeItemAtURL:temp error:nil]; return Fail(error,@"词典完整性校验失败"); } NSURL *target=[directory URLByAppendingPathComponent:@"msime-pinyin.db"];
    // Replace the dictionary in one step instead of deleting it and then moving the new one in.
    // Anything failing between those two - a full disk, a sandbox denial, a crash - leaves the user
    // with no dictionary at all, and what they had is not recoverable from here. replace keeps the
    // original until the new file is committed and puts it back if it is not. With nothing
    // installed yet there is nothing to replace, so the move is the whole operation.
    if ([fm fileExistsAtPath:target.path]) {
        NSError *replaceError=nil;
        if ([fm replaceItemAtURL:target withItemAtURL:temp backupItemName:@".msime-pinyin.db.previous"
                         options:NSFileManagerItemReplacementUsingNewMetadataOnly resultingItemURL:NULL
                           error:&replaceError]) return YES;
        // A failed replace leaves the staged file behind; it is ours and nothing else will collect it.
        [fm removeItemAtURL:temp error:nil];
        if (error) *error=replaceError;
        return NO;
    }
    if ([fm moveItemAtURL:temp toURL:target error:error]) return YES;
    [fm removeItemAtURL:temp error:nil];
    return NO;
}
