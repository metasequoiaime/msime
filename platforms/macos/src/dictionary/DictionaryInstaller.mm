#import "DictionaryInstaller.h"
#import <CommonCrypto/CommonDigest.h>
#import <sqlite3.h>
#include <sys/stat.h>
#include <errno.h>
static NSString *const MSIMEInstallerError = @"app.msime.client.dictionary-installer";
static BOOL Fail(NSError **e, NSString *s) { if (e) *e=[NSError errorWithDomain:MSIMEInstallerError code:1 userInfo:@{NSLocalizedDescriptionKey:s}]; return NO; }
static BOOL RejectSymlinkAncestors(NSURL *url) {
    NSString *current = url.URLByStandardizingPath.path;
    while (YES) {
        if ([current isEqual:@"/"] || [current isEqual:@"/var"] || [current isEqual:@"/tmp"]) return NO;
        struct stat info;
        if (lstat(current.fileSystemRepresentation, &info) == 0) {
            if ((info.st_mode & S_IFMT) == S_IFLNK) return YES;
        } else if (errno != ENOENT) {
            return YES;
        }
        NSString *parent = [current stringByDeletingLastPathComponent];
        if ([parent isEqual:current]) return NO;
        current = parent;
    }
}
BOOL MSIMEInstallDictionary(NSURL *source, NSURL *directory, NSString *expected, NSError **error) {
    if (!source.isFileURL || !directory.isFileURL || expected.length != CC_SHA256_DIGEST_LENGTH * 2) return Fail(error,@"词典参数无效");
    if (RejectSymlinkAncestors(directory)) return Fail(error,@"词典目录不可使用符号链接");
    NSData *data=[NSData dataWithContentsOfURL:source options:0 error:error]; if (!data) return NO;
    unsigned char digest[CC_SHA256_DIGEST_LENGTH]; CC_SHA256(data.bytes,(CC_LONG)data.length,digest); NSMutableString *actual=[NSMutableString string]; for (NSUInteger i=0;i<sizeof(digest);i++) [actual appendFormat:@"%02x",digest[i]]; if (![actual isEqualToString:expected.lowercaseString]) return Fail(error,@"词典指纹不匹配");
    sqlite3 *db=NULL; if (sqlite3_open_v2(source.fileSystemRepresentation,&db,SQLITE_OPEN_READONLY,NULL)!=SQLITE_OK) { sqlite3_close(db); return Fail(error,@"词典无法打开"); } sqlite3_stmt *stmt=NULL; BOOL valid=sqlite3_prepare_v2(db,"PRAGMA quick_check(1)",-1,&stmt,NULL)==SQLITE_OK && sqlite3_step(stmt)==SQLITE_ROW && strcmp((const char *)sqlite3_column_text(stmt,0),"ok")==0; sqlite3_finalize(stmt); sqlite3_close(db); if (!valid) return Fail(error,@"词典完整性校验失败");
    NSFileManager *fm=NSFileManager.defaultManager; if (![fm createDirectoryAtURL:directory withIntermediateDirectories:YES attributes:nil error:error]) return NO; NSURL *temp=[directory URLByAppendingPathComponent:@".msime.db.installing"]; [fm removeItemAtURL:temp error:nil]; if (![data writeToURL:temp options:NSDataWritingAtomic error:error]) return NO; NSURL *target=[directory URLByAppendingPathComponent:@"msime.db"];
    // Replace the dictionary in one step instead of deleting it and then moving the new one in.
    // Anything failing between those two - a full disk, a sandbox denial, a crash - leaves the user
    // with no dictionary at all, and what they had is not recoverable from here. replace keeps the
    // original until the new file is committed and puts it back if it is not. With nothing
    // installed yet there is nothing to replace, so the move is the whole operation.
    if ([fm fileExistsAtPath:target.path]) {
        NSError *replaceError=nil;
        if ([fm replaceItemAtURL:target withItemAtURL:temp backupItemName:@".msime.db.previous"
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
