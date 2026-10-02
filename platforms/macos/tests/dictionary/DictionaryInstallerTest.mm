#import <Foundation/Foundation.h>
#import <CommonCrypto/CommonDigest.h>
#import "../../src/dictionary/DictionaryInstaller.h"

#include <cassert>
#include <sqlite3.h>

// Installing a dictionary over the one the user already has.
//
// Every refusal here protects the same thing: the dictionary in place. A fingerprint that does not
// match is a download that went wrong or was tampered with, and a file SQLite cannot vouch for is
// one the Engine would fail to open later, in the middle of typing rather than here. Neither may
// cost the user what they already had.

static NSString *Digest(NSData *data) {
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(data.bytes, (CC_LONG)data.length, digest);
    NSMutableString *value = [NSMutableString string];
    for (NSUInteger index = 0; index < sizeof(digest); ++index) [value appendFormat:@"%02x", digest[index]];
    return value;
}

// A real SQLite database, because the installer runs quick_check against it rather than trusting
// the extension. The row makes the two fixtures distinguishable after an install.
static NSURL *Database(NSURL *directory, NSString *name, NSString *marker) {
    NSURL *url = [directory URLByAppendingPathComponent:name];
    sqlite3 *handle = NULL;
    assert(sqlite3_open_v2(url.fileSystemRepresentation, &handle,
                           SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, NULL) == SQLITE_OK);
    NSString *sql = [NSString stringWithFormat:@"CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('%@');", marker];
    assert(sqlite3_exec(handle, sql.UTF8String, NULL, NULL, NULL) == SQLITE_OK);
    assert(sqlite3_close(handle) == SQLITE_OK);
    return url;
}

static NSString *MarkerValue(NSURL *url) {
    sqlite3 *handle = NULL;
    if (sqlite3_open_v2(url.fileSystemRepresentation, &handle, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK) {
        sqlite3_close(handle);
        return nil;
    }
    sqlite3_stmt *statement = NULL;
    NSString *value = nil;
    if (sqlite3_prepare_v2(handle, "SELECT value FROM marker", -1, &statement, NULL) == SQLITE_OK &&
        sqlite3_step(statement) == SQLITE_ROW)
        value = @((const char *)sqlite3_column_text(statement, 0));
    sqlite3_finalize(statement);
    sqlite3_close(handle);
    return value;
}

int main() {
    @autoreleasepool {
        NSFileManager *files = NSFileManager.defaultManager;
        NSURL *root = [NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([files createDirectoryAtURL:root withIntermediateDirectories:YES attributes:nil error:nil]);
        NSURL *installed = [root URLByAppendingPathComponent:@"installed"];
        assert([files createDirectoryAtURL:installed withIntermediateDirectories:YES attributes:nil error:nil]);
        NSURL *target = [installed URLByAppendingPathComponent:@"msime.db"];

        NSURL *replacement = Database(root, @"replacement.db", @"new");
        NSData *replacementData = [NSData dataWithContentsOfURL:replacement];
        NSString *replacementDigest = Digest(replacementData);

        // Nothing installed yet: the first install is the whole operation.
        NSError *error = nil;
        assert(MSIMEInstallDictionary(replacement, installed, replacementDigest, &error) && !error);
        assert([MarkerValue(target) isEqual:@"new"]);
        // The staged file does not survive the install.
        assert(![files fileExistsAtPath:[installed URLByAppendingPathComponent:@".msime.db.installing"].path]);

        // Installing over it: the user ends up with the new one.
        NSURL *second = Database(root, @"second.db", @"newer");
        NSData *secondData = [NSData dataWithContentsOfURL:second];
        assert(MSIMEInstallDictionary(second, installed, Digest(secondData), &error) && !error);
        assert([MarkerValue(target) isEqual:@"newer"]);
        assert(![files fileExistsAtPath:[installed URLByAppendingPathComponent:@".msime.db.installing"].path]);

        // A fingerprint that does not match is refused, and what is installed is untouched. This is
        // the case the atomic replace exists for: the old rule deleted the target before moving the
        // new file into place, so anything failing in between left no dictionary at all.
        error = nil;
        assert(!MSIMEInstallDictionary(replacement, installed, Digest(secondData), &error) && error);
        assert([MarkerValue(target) isEqual:@"newer"]);

        // A file SQLite cannot vouch for is refused before it can replace anything. A file cut short
        // keeps a valid header - it opens - and fails only when the pages are actually read, which
        // is what the integrity check is there for and what the Engine would otherwise hit in the
        // middle of typing.
        NSURL *damaged = Database(root, @"damaged.db", @"broken");
        NSMutableData *bytes = [[NSData dataWithContentsOfURL:damaged] mutableCopy];
        assert(bytes.length > 2048);
        [bytes setLength:bytes.length - 1024];
        assert([bytes writeToURL:damaged options:NSDataWritingAtomic error:nil]);
        error = nil;
        assert(!MSIMEInstallDictionary(damaged, installed, Digest(bytes), &error) && error);
        assert([MarkerValue(target) isEqual:@"newer"]);

        // Arguments that cannot describe an install at all, including a digest of the wrong length -
        // a truncated one would otherwise be compared against a prefix of the real value.
        error = nil;
        assert(!MSIMEInstallDictionary(replacement, installed, [replacementDigest substringToIndex:63], &error) && error);
        assert(!MSIMEInstallDictionary(replacement, installed, @"", nil));
        assert(!MSIMEInstallDictionary([NSURL URLWithString:@"https://example.invalid/msime.db"], installed,
                                       replacementDigest, nil));
        assert(!MSIMEInstallDictionary(replacement, [NSURL URLWithString:@"https://example.invalid/"],
                                       replacementDigest, nil));
        assert([MarkerValue(target) isEqual:@"newer"]);

        // A source that is not there at all fails without leaving anything staged behind.
        error = nil;
        NSURL *missing = [root URLByAppendingPathComponent:@"missing.db"];
        assert(!MSIMEInstallDictionary(missing, installed, replacementDigest, &error) && error);
        assert(![files fileExistsAtPath:[installed URLByAppendingPathComponent:@".msime.db.installing"].path]);
        assert([MarkerValue(target) isEqual:@"newer"]);

        // A destination directory supplied through preferences must not redirect an install through a symlink.
        NSURL *outside = [root URLByAppendingPathComponent:@"outside"];
        assert([files createDirectoryAtURL:outside withIntermediateDirectories:YES attributes:nil error:nil]);
        NSURL *linked = [root URLByAppendingPathComponent:@"linked-installed"];
        assert([files createSymbolicLinkAtURL:linked withDestinationURL:outside error:nil]);
        error = nil;
        assert(!MSIMEInstallDictionary(replacement, linked, replacementDigest, &error) && error);
        assert(![files fileExistsAtPath:[outside URLByAppendingPathComponent:@"msime.db"].path]);

        [files removeItemAtURL:root error:nil];
    }
    return 0;
}
