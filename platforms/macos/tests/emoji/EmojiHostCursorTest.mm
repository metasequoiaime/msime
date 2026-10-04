#import "MSIMEClientSession.h"
#import <Foundation/Foundation.h>
#include <sqlite3.h>
#include <cassert>

int main() {
    @autoreleasepool {
        NSFileManager *files = NSFileManager.defaultManager;
        NSURL *directory = [NSURL fileURLWithPath:[NSTemporaryDirectory()
            stringByAppendingPathComponent:[@"msime-host-cursor-" stringByAppendingString:NSUUID.UUID.UUIDString]]];
        NSError *error = nil;
        assert([files createDirectoryAtURL:directory withIntermediateDirectories:NO attributes:nil error:&error]);
        @try {
            sqlite3 *db = nullptr;
            assert(sqlite3_open([[directory URLByAppendingPathComponent:@"msime-others.db"].path fileSystemRepresentation], &db) == SQLITE_OK);
            const char *sql =
                "CREATE TABLE emoji(emoji TEXT,category TEXT,keywords TEXT,pinyin TEXT,sort_order INTEGER);"
                "CREATE TABLE kaomoji_catalog(kaomoji TEXT,keywords TEXT,sort_order INTEGER);"
                "CREATE TABLE symbol_catalog(symbol TEXT,category TEXT,parent_category TEXT,keywords TEXT,sort_order INTEGER);"
                "INSERT INTO emoji VALUES(NULL,'fixture','match','',0),('','fixture','match','',1),"
                "('synthetic-same','fixture','match','',2),('synthetic-same','fixture','match','',3),"
                "('synthetic-tail','fixture','match','',4);"
                "INSERT INTO kaomoji_catalog SELECT emoji,keywords,sort_order FROM emoji;"
                "INSERT INTO symbol_catalog SELECT emoji,category,'parent',keywords,sort_order FROM emoji;";
            assert(sqlite3_exec(db, sql, nullptr, nullptr, nullptr) == SQLITE_OK);
            assert(sqlite3_close(db) == SQLITE_OK);
            for (NSString *category in @[@"", @"kaomoji", @"symbols"]) {
                auto request = ^NSDictionary *(NSUInteger offset, BOOL cursor) {
                    return [MSIMEClientSession emojiCatalogRequest:@{
                        @"resources": directory.path, @"category": category, @"search": @"match",
                        @"offset": @(offset), @"limit": @2, @"cursor": @(cursor)
                    }];
                };
                NSDictionary *empty = request(0, YES);
                assert(!empty[@"error"] && [empty[@"items"] count] == 0);
                assert([empty[@"next_offset"] isEqual:@2] && [empty[@"complete"] isEqual:@NO]);
                NSDictionary *duplicates = request(2, YES);
                NSArray *items = duplicates[@"items"];
                assert(!duplicates[@"error"] && items.count == 2);
                assert([items[0][@"text"] isEqual:@"synthetic-same"] && [items[0] isEqual:items[1]]);
                assert([duplicates[@"next_offset"] isEqual:@4] && [duplicates[@"complete"] isEqual:@NO]);
                NSDictionary *tail = request(4, YES);
                assert([tail[@"items"] count] == 1 && [tail[@"next_offset"] isEqual:@5]);
                assert([tail[@"complete"] isEqual:@YES]);
                NSDictionary *end = request(5, YES);
                assert([end[@"items"] count] == 0 && [end[@"next_offset"] isEqual:@5]);
                assert([end[@"complete"] isEqual:@YES]);
                NSDictionary *legacy = request(2, NO);
                assert(!legacy[@"error"] && [legacy[@"items"] count] == 1);
                assert(!legacy[@"next_offset"] && !legacy[@"complete"]);
            }
            NSDictionary *invalid = [MSIMEClientSession emojiCatalogRequest:@{@"resources": @"relative", @"cursor": @YES}];
            assert(invalid[@"error"] != nil);
            puts("Native Objective-C/Rust/SQLite emoji cursor integration passed");
        } @finally {
            assert([files removeItemAtURL:directory error:&error]);
        }
    }
    return 0;
}
