#import "TextClient.h"
#import <AppKit/AppKit.h>
#import "MSIMEClientSession.h"
#include "msime_client.h"
#include <cassert>
#include <sqlite3.h>
#include <string>

@interface FakeTextClient : NSObject <MSIMETextClient>
@property(nonatomic, copy) NSString *committed;
// Whatever setMarkedText: was handed. A plain string for an ordinary composition, an
// attributed one once a phrase piece leads it, which is what the clause styling needs.
@property(nonatomic, strong) id marked;
@property(nonatomic) NSRange selection;
@property(nonatomic, strong) NSMutableArray<NSString *> *events;
@property(nonatomic) NSRange documentSelection;
@property(nonatomic, copy) NSString *following;
@property(nonatomic, copy) NSString *document;
- (NSString *)markedString;
@end
@implementation FakeTextClient
- (void)insertText:(id)text replacementRange:(NSRange)range { assert(range.location == NSNotFound); if (!self.events) self.events = [NSMutableArray array]; [self.events addObject:@"commit"]; self.committed = text; }
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement { assert(replacement.location == NSNotFound); if (!self.events) self.events = [NSMutableArray array]; [self.events addObject:@"marked"]; self.marked = text; self.selection = selection; }
// The text without its attributes, for the assertions that only care what it says.
- (NSString *)markedString { return [self.marked isKindOfClass:NSAttributedString.class] ? [(NSAttributedString *)self.marked string] : self.marked; }
- (NSRange)selectedRange { return self.documentSelection; }
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    if (self.following) {
        if (range.location != self.documentSelection.location || range.length != 1) return nil;
        return [[NSAttributedString alloc] initWithString:self.following];
    }
    if (!self.document || range.location == NSNotFound || range.location > self.document.length ||
        range.length > self.document.length - range.location) return nil;
    return [[NSAttributedString alloc] initWithString:[self.document substringWithRange:range]];
}
@end

static NSDictionary *MaintenanceCandidate(MSIMEClientSession *session) {
    for (NSDictionary *candidate in [session viewWithError:nil][@"candidates"])
        if ([candidate[@"text"] isEqual:@"拟好"]) return candidate[@"id"];
    return nil;
}

static void TestCustomTranslationHTTPBridge() {
    NSError *error = nil;
    NSDictionary *request = @{@"config":@{@"enabled":@YES, @"endpoint":@"https://translation.invalid/api", @"api_key":@""},
        @"text":@"hello", @"source_language":@"en", @"target_language":@"zh"};
    NSDictionary *descriptor = [MSIMEClientSession customTranslationHTTPRequest:request error:&error];
    assert(descriptor && !error && [descriptor[@"method"] isEqual:@"POST"]);
    assert([descriptor[@"body"][@"source_lang"] isEqual:@"EN"] && !descriptor[@"headers"][@"Authorization"]);
    assert([[MSIMEClientSession parseCustomTranslationResponse:[@"{\"data\":\"测试释义\"}" dataUsingEncoding:NSUTF8StringEncoding] error:&error] isEqual:@"测试释义"] && !error);
    assert(![MSIMEClientSession parseCustomTranslationResponse:[@"invalid" dataUsingEncoding:NSUTF8StringEncoding] error:&error] && !error);
    assert(![MSIMEClientSession parseCustomTranslationResponse:NSData.data error:&error] && !error);
    assert(![MSIMEClientSession parseCustomTranslationResponse:[NSMutableData dataWithLength:1048577] error:&error] && error);
    error = nil;
    NSMutableDictionary *disabled = [request mutableCopy];
    disabled[@"config"] = @{@"enabled":@NO};
    assert(![MSIMEClientSession customTranslationHTTPRequest:disabled error:&error] && !error);
}

static void TestTencentTranslationHTTPBridge() {
    NSError *error = nil;
    NSDictionary *request = @{@"config":@{@"enabled":@YES, @"secret_id":@"AKIDsynthetic", @"secret_key":@"synthetic", @"region":@""},
        @"texts":@[@"测试"], @"source_language":@"zh", @"target_language":@"en", @"timestamp":@1704067200};
    NSDictionary *descriptor = [MSIMEClientSession tencentTranslationHTTPRequest:request error:&error];
    assert(descriptor && !error && [descriptor[@"url"] isEqual:@"https://tmt.tencentcloudapi.com"]);
    assert([descriptor[@"headers"][@"Authorization"] containsString:@"/2024-01-01/tmt/tc3_request"]);
    assert([descriptor[@"headers"][@"X-TC-Timestamp"] isEqual:@"1704067200"]);
    assert([descriptor[@"headers"][@"X-TC-Region"] isEqual:@"ap-guangzhou"]);
    NSData *payload = [descriptor[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *body = [NSJSONSerialization JSONObjectWithData:payload options:0 error:&error];
    assert(!error && [body[@"SourceTextList"] isEqual:@[@"测试"]]);
    NSData *response = [@"{\"Response\":{\"TargetTextList\":[\" test \",\"\"]}}" dataUsingEncoding:NSUTF8StringEncoding];
    assert(([[MSIMEClientSession parseTencentTranslationResponse:response expectedCount:2 error:&error] isEqual:@[@"test", NSNull.null]]));
    assert(!error);
    assert(![MSIMEClientSession parseTencentTranslationResponse:response expectedCount:1 error:&error] && !error);
    assert(![MSIMEClientSession parseTencentTranslationResponse:NSData.data expectedCount:1 error:&error] && !error);
    assert(![MSIMEClientSession parseTencentTranslationResponse:response expectedCount:10 error:&error] && error);
    error = nil;
    NSMutableDictionary *disabled = [request mutableCopy]; disabled[@"config"] = @{@"enabled":@NO};
    assert(![MSIMEClientSession tencentTranslationHTTPRequest:disabled error:&error] && !error);
}

static void TestEngineMaintenance() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":@{@"scheme":@"quanpin", @"default_ime_mode":@"chinese", @"candidate_page_size":@5, @"learning":@NO, @"chinese_punctuation":@YES}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    sqlite3 *database = nullptr;
    assert(sqlite3_open([[options[@"dictionaries"] stringByAppendingPathComponent:@"msime.db"] fileSystemRepresentation], &database) == SQLITE_OK);
    assert(sqlite3_exec(database, "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
        "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',10000),('ni''hao','nh','拟好',9000);", nullptr, nullptr, nullptr) == SQLITE_OK);
    assert(sqlite3_close(database) == SQLITE_OK);
    assert(sqlite3_open([[options[@"dictionaries"] stringByAppendingPathComponent:@"english.db"] fileSystemRepresentation], &database) == SQLITE_OK);
    assert(sqlite3_exec(database, "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);"
        "INSERT INTO english_words VALUES('hello','hello',100);"
        "CREATE TABLE en_zh_glosses(english TEXT COLLATE BINARY PRIMARY KEY,chinese_gloss TEXT NOT NULL) WITHOUT ROWID;"
        "CREATE TABLE zh_en_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY,english_gloss TEXT NOT NULL) WITHOUT ROWID;"
        "INSERT INTO en_zh_glosses VALUES('hello','测试释义');", nullptr, nullptr, nullptr) == SQLITE_OK);
    assert(sqlite3_close(database) == SQLITE_OK);
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error && [session setFocused:YES error:&error]);
    assert(![session translationQueryWithError:&error] && !error);
    NSDictionary *englishView = [session setDedicatedEnglishEnabled:YES error:&error];
    assert(!error && [englishView[@"dedicated_english"] isEqual:@YES]);
    assert([session typeASCII:'h' shift:NO error:&error]);
    NSDictionary *englishTyped = [session typeASCII:'e' shift:NO error:&error];
    assert(!error && [englishTyped[@"view"][@"dedicated_english"] isEqual:@YES]);
    assert([englishTyped[@"view"][@"candidates"][0][@"text"] isEqual:@"hello"]);
    NSDictionary *translationQuery = [session translationQueryWithError:&error];
    assert(translationQuery && !error);
    NSDictionary *glossRequest = @{@"generation":translationQuery[@"generation"], @"candidates":@[@{@"text":@"hello", @"source":@4}]};
    __block NSDictionary *gloss = nil;
    __block NSError *glossError = nil;
    dispatch_semaphore_t finished = dispatch_semaphore_create(0);
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        assert(!NSThread.isMainThread);
        gloss = [MSIMEClientSession candidateGlossRequest:glossRequest resources:options[@"dictionaries"] error:&glossError];
        dispatch_semaphore_signal(finished);
    });
    assert(dispatch_semaphore_wait(finished, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    assert(gloss && !glossError && [gloss[@"generation"] isEqual:translationQuery[@"generation"]]);
    uint64_t translationGeneration = [translationQuery[@"generation"] unsignedLongLongValue];
    NSDictionary *translated = [session applyTranslations:gloss[@"translations"] generation:translationGeneration error:&error];
    assert(!error && [translated[@"applied"] isEqual:@YES]);
    assert([translated[@"view"][@"candidates"][0][@"translation"] isEqual:@"测试释义"]);
    assert([translated[@"view"][@"candidates"][0][@"id"] isEqual:englishTyped[@"view"][@"candidates"][0][@"id"]]);
    assert([translated[@"view"][@"editing_text"] isEqual:englishTyped[@"view"][@"editing_text"]]);
    assert(![MSIMEClientSession candidateGlossRequest:glossRequest resources:@"relative" error:&error] && error);
    error = nil;
    assert(![MSIMEClientSession candidateGlossRequest:@{@"padding":[@"x" stringByPaddingToLength:262145 withString:@"x" startingAtIndex:0]} resources:options[@"dictionaries"] error:&error] && error);
    error = nil;
    // Non-English glosses come from an offline dictionary installed beside the resource directory, one file per target language.
    NSString *offlineGlosses = [[options[@"dictionaries"] stringByDeletingLastPathComponent] stringByAppendingPathComponent:@"offline-glosses"];
    assert([NSFileManager.defaultManager createDirectoryAtPath:offlineGlosses withIntermediateDirectories:YES attributes:nil error:nil]);
    assert(sqlite3_open([[offlineGlosses stringByAppendingPathComponent:@"zh-fr.db"] fileSystemRepresentation], &database) == SQLITE_OK);
    assert(sqlite3_exec(database, "PRAGMA user_version=1;"
        "CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY,gloss TEXT NOT NULL,source TEXT NOT NULL) WITHOUT ROWID;"
        "CREATE TABLE meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);"
        "INSERT INTO meta VALUES('target_language','fr');"
        "INSERT INTO zh_glosses VALUES('你好','bonjour','hello');", nullptr, nullptr, nullptr) == SQLITE_OK);
    assert(sqlite3_close(database) == SQLITE_OK);
    NSDictionary *frenchRequest = @{@"generation":@7, @"target_language":@"fr", @"candidates":@[@{@"text":@"你好", @"source":@0}, @{@"text":@"hello", @"source":@4}]};
    NSDictionary *french = [MSIMEClientSession candidateGlossRequest:frenchRequest resources:options[@"dictionaries"] error:&error];
    assert(french && !error && [french[@"generation"] isEqual:@7]);
    assert(([french[@"translations"] isEqual:@[@{@"text":@"你好", @"translation":@"bonjour"}]]));
    NSMutableDictionary *germanRequest = [frenchRequest mutableCopy];
    germanRequest[@"target_language"] = @"de";
    NSDictionary *german = [MSIMEClientSession candidateGlossRequest:germanRequest resources:options[@"dictionaries"] error:&error];
    assert(german && !error && [german[@"translations"] isEqual:@[]]);
    assert((![session applyTranslations:@[@{@"text":@"hello", @"translation":[@"x" stringByPaddingToLength:4097 withString:@"x" startingAtIndex:0]}] generation:translationGeneration error:&error] && error));
    error = nil;
    assert([[session setCharacterWidthFull:YES error:&error][@"character_width"] isEqual:@"Fullwidth"]);
    assert([[session command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:@"ｈｅｌｌｏ"]);
    translated = [session applyTranslations:gloss[@"translations"] generation:translationGeneration error:&error];
    assert(!error && [translated[@"applied"] isEqual:@NO]);
    assert([[session setCharacterWidthFull:NO error:&error][@"character_width"] isEqual:@"Halfwidth"]);
    for (NSNumber *enabled in @[@YES, @NO, @YES]) {
        assert([session setEnglishMode:enabled.boolValue error:&error] && !error);
        __block NSUInteger replacements = 0;
        id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEClientSessionDidReplaceSnapshotNotification object:session queue:nil usingBlock:^(NSNotification *notification) {
            assert(notification.object == session);
            // Mode restoration must precede the notification consumed by the IMK host.
            assert([[session viewWithError:nil][@"dedicated_english"] isEqual:enabled]);
            ++replacements;
        }];
        NSError *activationError = nil;
        NSString *version = [@"" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0];
        assert(![MSIMEClientSession applySnapshotHandle:UINT64_MAX expectedVersion:version error:&activationError]);
        assert(activationError && replacements == 1);
        [NSNotificationCenter.defaultCenter removeObserver:observer];
        NSDictionary *restored = [session viewWithError:&error];
        assert(restored && !error && [restored[@"dedicated_english"] isEqual:enabled]);
        assert([session setFocused:YES error:&error]);
        if (enabled.boolValue) {
            assert([session typeASCII:'h' shift:NO error:&error]);
            NSDictionary *typed = [session typeASCII:'e' shift:NO error:&error];
            assert([typed[@"view"][@"candidates"][0][@"text"] isEqual:@"hello"]);
            assert([[session command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:@"hello"]);
        }
    }
    englishView = [session setDedicatedEnglishEnabled:NO error:&error];
    assert(!error && [englishView[@"dedicated_english"] isEqual:@NO]);
    assert(![session onlineQueryWithError:&error] && !error);
    for (char key : std::string("nihao")) assert([session typeASCII:key shift:NO error:&error]);
    NSDictionary *query = [session onlineQueryWithError:&error];
    assert(query && !error);
    NSString *url = [MSIMEClientSession cloudRequestURLForQuery:query error:&error];
    assert([url hasPrefix:@"https://inputtools.google.com/"] && !error);
    NSData *body = [@"[\"SUCCESS\", [[\"nihao\", [\"云端测试候选\"]]]]" dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *beforeCloud = [session viewWithError:&error];
    NSDictionary *cloud = [session applyCloudResponse:[@"invalid" dataUsingEncoding:NSUTF8StringEncoding] query:query error:&error];
    assert(!error && [cloud[@"applied"] isEqual:@NO] && [cloud[@"view"] isEqual:beforeCloud]);
    cloud = [session applyCloudResponse:body query:query error:&error];
    assert(!error && [cloud[@"applied"] isEqual:@YES]);
    assert([cloud[@"view"][@"editing_text"] isEqual:beforeCloud[@"editing_text"]]);
    NSDictionary *afterCloud = [session viewWithError:&error];
    cloud = [session applyCloudResponse:body query:[session onlineQueryWithError:&error] error:&error];
    assert(!error && [cloud[@"applied"] isEqual:@NO] && [cloud[@"view"] isEqual:afterCloud]);
    assert([session command:MSIME_CANCEL error:&error]);
    cloud = [session applyCloudResponse:body query:query error:&error];
    assert(!error && [cloud[@"applied"] isEqual:@NO]);
    assert(![session applyCloudResponse:[NSMutableData dataWithLength:262145] query:query error:&error] && error);
    error = nil;
    assert(![MSIMEClientSession cloudRequestURLForQuery:@{@"padding":[@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0]} error:&error] && error);
    error = nil;
    for (char key : std::string("nihao")) assert([session typeASCII:key shift:NO error:&error]);
    NSDictionary *identifier = MaintenanceCandidate(session);
    assert(identifier && !error);
    uint64_t generation = [identifier[@"generation"] unsignedLongLongValue];
    NSUInteger index = [identifier[@"index"] unsignedIntegerValue];
    assert(![session pinGeneration:generation + 1 index:index error:&error] && error);
    error = nil;
    NSDictionary *result = [session pinGeneration:generation index:index error:&error];
    assert(result && !error && [result[@"handled"] boolValue]);
    assert([result[@"view"][@"candidates"][0][@"text"] isEqual:@"拟好"]);
    identifier = MaintenanceCandidate(session);
    result = [session fixGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] position:2 error:&error];
    assert(result && !error && [result[@"handled"] boolValue]);
    assert([result[@"view"][@"candidates"][1][@"text"] isEqual:@"拟好"]);
    assert([result[@"view"][@"candidates"][1][@"fixed_position"] isEqual:@2]);
    identifier = MaintenanceCandidate(session);
    result = [session clearPositionGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] error:&error];
    assert(result && !error && [result[@"handled"] boolValue]);
    identifier = MaintenanceCandidate(session);
    result = [session removeGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] error:&error];
    assert(result && !error && [result[@"handled"] boolValue] && !MaintenanceCandidate(session));
    assert([session closeWithError:&error] && !error);
    session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error && [session setFocused:YES error:&error]);
    for (char key : std::string("nihao")) assert([session typeASCII:key shift:NO error:&error]);
    assert(!MaintenanceCandidate(session));
    assert([session closeWithError:&error] && !error);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
}

static void TestEngineEdges(FakeTextClient *client) {
    for (NSString *code in @[@"4e2d", @"20000", @"41"]) {
        for (uint8_t edge = 0; edge < 2; ++edge) {
            NSError *error = nil;
            NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
            NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
            for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
                NSString *path = [root stringByAppendingPathComponent:name];
                assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
                options[name] = path;
            }
            MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
            assert(session && !error && [session setFocused:YES error:&error]);
            assert([session typeASCII:'U' shift:YES error:&error]);
            for (NSUInteger i = 0; i < code.length; ++i) assert([session typeASCII:[code characterAtIndex:i] shift:NO error:&error]);
            NSDictionary *view = [session viewWithError:&error];
            NSDictionary *identifier = [view[@"candidates"] firstObject][@"id"];
            assert(identifier && !error);
            assert(![session selectEdgeGeneration:[identifier[@"generation"] unsignedLongLongValue] + 1 index:[identifier[@"index"] unsignedIntegerValue] edge:edge error:&error]);
            assert(error);
            error = nil;
            assert([[[session viewWithError:&error] objectForKey:@"editing_text"] isEqual:view[@"editing_text"]]);
            NSDictionary *result = [session selectEdgeGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] edge:edge error:&error];
            assert(result && !error);
            if ([code isEqual:@"41"]) {
                assert(![result[@"handled"] boolValue]);
                assert([result[@"view"][@"editing_text"] isEqual:view[@"editing_text"]]);
                // The host then falls back to punctuation, as Windows does for a Normal reply: the highlighted candidate is committed followed by the key's punctuation.
                client.committed = nil;
                NSDictionary *fallback = [session punctuation:edge ? ']' : '[' error:&error];
                assert(fallback && !error && [fallback[@"handled"] boolValue]);
                MSIMEApplyTransition(fallback, client);
                assert([client.committed isEqual:edge ? @"A】" : @"A【"]);
                assert([[[session viewWithError:&error] objectForKey:@"editing_text"] length] == 0);
            } else {
                MSIMEApplyTransition(result, client);
                assert([client.committed isEqual:[code isEqual:@"4e2d"] ? @"中" : @"𠀀"]);
                assert([client.markedString length] == 0);
            }
            assert([session closeWithError:&error]);
            assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
        }
    }
}

static void TestEnginePreedit(FakeTextClient *client) {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"shuangpin", @"default_ime_mode": @"chinese", @"shuangpin_profile": @"microsoft", @"shuangpin_preedit_uses_raw": @YES, @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSError *error = nil;
    NSMutableDictionary *startupPreferences = [options[@"preferences"] mutableCopy];
    options[@"preferences"] = startupPreferences;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error && [session setFocused:YES error:&error]);
    startupPreferences[@"scheme"] = @"quanpin";
    startupPreferences[@"shuangpin_profile"] = @"xiaohe";
    NSError *startupRecoveryError = nil;
    NSString *syntheticVersion = [@"" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0];
    assert(![MSIMEClientSession applySnapshotHandle:UINT64_MAX expectedVersion:syntheticVersion error:&startupRecoveryError]);
    assert(startupRecoveryError);
    NSDictionary *startupRecovered = [session viewWithError:&error];
    assert(startupRecovered && !error && [startupRecovered[@"scheme"] isEqual:@1] && [startupRecovered[@"shuangpin_profile"] isEqual:@"microsoft"]);
    assert([session setFocused:YES error:&error]);
    NSDictionary *shared = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(shared && !error);
    NSUInteger revision = 0;
    for (NSNumber *raw in @[@YES, @NO, @YES]) {
        NSMutableDictionary *preferences = [shared[@"preferences"] mutableCopy];
        preferences[@"scheme"] = @"shuangpin";
        preferences[@"default_ime_mode"] = @"chinese";
        preferences[@"shuangpin_profile"] = @"microsoft";
        preferences[@"shuangpin_preedit_uses_raw"] = raw;
        NSDictionary *snapshot = @{@"format_version": @1, @"revision": @(++revision), @"preferences": preferences};
        assert([[session updatePreferencesSnapshot:snapshot error:&error][@"deferred"] isEqual:@NO]);
        assert([session typeASCII:'b' shift:NO error:&error]);
        NSDictionary *typed = [session typeASCII:';' shift:NO error:&error];
        assert(typed && !error && [typed[@"view"][@"editing_text"] isEqual:@"b;"]);
        assert([typed[@"view"][@"preedit"] isEqual:raw.boolValue ? @"b;" : @"bing"]);
        MSIMEApplyTransition(typed, client);
        assert([client.markedString isEqual:typed[@"view"][@"preedit"]] && client.selection.location == [client.markedString length]);
        NSDictionary *punctuationView = [session setChinesePunctuationEnabled:NO error:&error];
        assert(punctuationView && !error && !punctuationView[@"view"]);
        assert([punctuationView[@"editing_text"] isEqual:@"b;"] && [punctuationView[@"preedit"] isEqual:typed[@"view"][@"preedit"]]);
        assert([punctuationView[@"generation"] isEqual:typed[@"view"][@"generation"]]);
        MSIMEApplyTransition(@{@"view":punctuationView}, client);
        assert([client.markedString isEqual:typed[@"view"][@"preedit"]]);
        assert([session setChinesePunctuationEnabled:YES error:&error] && !error);
        preferences[@"shuangpin_preedit_uses_raw"] = @(!raw.boolValue);
        assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"shuangpin_preedit_uses_raw"] isEqual:raw]);
        NSDictionary *pending = @{@"format_version": @1, @"revision": @(++revision), @"preferences": preferences};
        NSDictionary *deferred = [session updatePreferencesSnapshot:pending error:&error];
        assert([deferred[@"deferred"] isEqual:@YES] && !error);
        MSIMEApplyTransition(deferred, client);
        assert([client.markedString isEqual:typed[@"view"][@"preedit"]]);
        NSDictionary *cancelled = [session command:MSIME_CANCEL error:&error];
        assert(cancelled && !error);
        MSIMEApplyTransition(cancelled, client);
        assert([client.markedString length] == 0 && client.selection.location == 0);
        assert([[session updatePreferencesSnapshot:pending error:&error][@"deferred"] isEqual:@NO]);
        assert([session typeASCII:'b' shift:NO error:&error]);
        NSDictionary *updated = [session typeASCII:';' shift:NO error:&error];
        assert(updated && !error);
        MSIMEApplyTransition(updated, client);
        assert([client.markedString isEqual:raw.boolValue ? @"bing" : @"b;"]);
        MSIMEApplyTransition([session command:MSIME_CANCEL error:&error], client);
        assert(!error && [client.markedString length] == 0);
    }
    NSError *staleError = nil;
    assert((![session updatePreferencesSnapshot:@{@"format_version": @1, @"revision": @0, @"preferences": shared[@"preferences"]} error:&staleError]));
    assert(staleError && [[MSIMEClientSession activeHostOptions][@"preferences"][@"shuangpin_preedit_uses_raw"] isEqual:@NO]);
    assert([session typeASCII:'b' shift:NO error:&error]);
    __block NSUInteger replacements = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEClientSessionDidReplaceSnapshotNotification object:session queue:nil usingBlock:^(NSNotification *note) {
        assert(note.object == session && NSThread.isMainThread);
        ++replacements;
    }];
    NSString *version = [@"" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0];
    // Activation is refused outright while a composition is live, before anything is destroyed. That guard
    // arrived after the recovery assertion below and this caller was never updated, which is why the run
    // aborted here: the recovery path cannot be reached from a composing session at all any more.
    NSError *composingError = nil;
    assert(![MSIMEClientSession applySnapshotHandle:UINT64_MAX expectedVersion:version error:&composingError]);
    assert(composingError && replacements == 0);
    // Refused means untouched, not half-applied: the composition the user is still typing survives.
    assert([[session viewWithError:&error][@"editing_text"] isEqual:@"b"] && !error);

    // With the composition finished the guard opens, and a nonexistent prepared handle forces activation
    // failure after destruction - exercising actual host recovery rather than a mocked notification.
    MSIMEApplyTransition([session command:MSIME_CANCEL error:&error], client);
    assert(!error);
    NSError *activationError = nil;
    assert(![MSIMEClientSession applySnapshotHandle:UINT64_MAX expectedVersion:version error:&activationError]);
    assert(activationError && replacements == 1);
    NSDictionary *recovered = [session viewWithError:&error];
    assert(recovered && !error && [recovered[@"editing_text"] length] == 0);
    assert([session setFocused:YES error:&error]);
    NSDictionary *afterRecovery = [session typeASCII:'b' shift:NO error:&error];
    assert(afterRecovery && !error && [afterRecovery[@"view"][@"editing_text"] isEqual:@"b"]);
    MSIMEApplyTransition(afterRecovery, client);
    assert([client.markedString isEqual:afterRecovery[@"view"][@"preedit"]]);
    NSDictionary *expandedAfterRecovery = [session typeASCII:';' shift:NO error:&error];
    assert(expandedAfterRecovery && !error);
    // The last accepted preference was formatted display, not the raw startup value.
    assert([expandedAfterRecovery[@"view"][@"preedit"] isEqual:@"bing"]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    NSMutableDictionary *otherOptions = [[MSIMEClientSession activeHostOptions] mutableCopy];
    NSMutableDictionary *otherPreferences = [otherOptions[@"preferences"] mutableCopy];
    otherPreferences[@"scheme"] = @"quanpin";
    otherOptions[@"preferences"] = otherPreferences;
    MSIMEClientSession *other = [[MSIMEClientSession alloc] initWithOptions:otherOptions error:&error];
    assert(other && !error);
    assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"scheme"] isEqual:@"shuangpin"]);
    assert([other setFocused:YES error:&error]);
    assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"scheme"] isEqual:@"quanpin"]);
    NSMutableDictionary *formattedOptions = [otherOptions mutableCopy];
    NSMutableDictionary *formattedPreferences = [otherPreferences mutableCopy];
    formattedPreferences[@"scheme"] = @"shuangpin";
    formattedPreferences[@"shuangpin_profile"] = @"microsoft";
    formattedPreferences[@"shuangpin_preedit_uses_raw"] = @NO;
    formattedOptions[@"preferences"] = formattedPreferences;
    MSIMEClientSession *formatted = [[MSIMEClientSession alloc] initWithOptions:formattedOptions error:&error];
    assert(formatted && !error && [formatted setFocused:YES error:&error]);
    for (NSString *raw in @[@"nini", @"ni'ni"]) {
        for (NSUInteger i = 0; i < raw.length; ++i)
            assert([formatted typeASCII:[raw characterAtIndex:i] shift:NO error:&error]);
        NSDictionary *segmented = [formatted viewWithError:&error];
        assert(!error && [segmented[@"preedit"] isEqual:@"ni'ni"]);
        NSArray *offsets = [raw isEqual:@"nini"] ? @[@0, @1, @2, @4, @5] : @[@0, @1, @2, @3, @4, @5];
        for (NSUInteger i = raw.length; i > 0; --i) {
            NSDictionary *moved = [formatted command:MSIME_MOVE_LEFT error:&error];
            assert(moved && !error && [moved[@"view"][@"caret_position"] unsignedIntegerValue] == i - 1);
            MSIMEApplyTransition(moved, client);
            assert([client.markedString isEqual:@"ni'ni"] && client.selection.location == [offsets[i - 1] unsignedIntegerValue]);
        }
        for (NSUInteger i = 1; i <= raw.length; ++i) {
            NSDictionary *moved = [formatted command:MSIME_MOVE_RIGHT error:&error];
            assert(moved && !error && [moved[@"view"][@"caret_position"] unsignedIntegerValue] == i);
            MSIMEApplyTransition(moved, client);
            assert([client.markedString isEqual:@"ni'ni"] && client.selection.location == [offsets[i] unsignedIntegerValue]);
        }
        MSIMEApplyTransition([formatted command:MSIME_CANCEL error:&error], client);
        assert(!error && ![client.markedString length]);
    }
    assert([formatted closeWithError:&error] && !error);
    assert([session setFocused:YES error:&error]);
    assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"scheme"] isEqual:@"shuangpin"]);
    assert([session setFocused:NO error:&error]);
    assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"scheme"] isEqual:@"shuangpin"]);
    assert([other closeWithError:&error]);
    NSError *closedError = nil;
    assert(![other setFocused:YES error:&closedError] && closedError);
    assert([[MSIMEClientSession activeHostOptions][@"preferences"][@"scheme"] isEqual:@"shuangpin"]);
    assert([session closeWithError:&error] && !error);
    assert([MSIMEClientSession activeHostOptions][@"error"] != nil);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
}

// A focus change applies an empty composition to a client that is blocked waiting for this input method, so clearing marked text that was never set must not call the client at all.
static void TestTrackedMarkedText() {
    FakeTextClient *client = [FakeTextClient new];
    client.events = [NSMutableArray array];
    NSDictionary *idle = @{@"commit": NSNull.null, @"view": @{@"editing_text": @"", @"caret_position": @0}};
    BOOL hasMarked = NO;
    MSIMEApplyTransitionTrackingMarkedText(idle, client, MSIMEInlinePreeditStylePinyin, nil, &hasMarked);
    assert(client.events.count == 0 && !hasMarked);
    // A composition is written and remembered, and the clear that ends it still goes out.
    MSIMEApplyTransitionTrackingMarkedText(@{@"view": @{@"editing_text": @"ni", @"caret_position": @2}}, client,
                                           MSIMEInlinePreeditStylePinyin, nil, &hasMarked);
    assert([client.markedString isEqual:@"ni"] && hasMarked);
    MSIMEApplyTransitionTrackingMarkedText(idle, client, MSIMEInlinePreeditStylePinyin, nil, &hasMarked);
    assert([client.events isEqual:(@[@"marked", @"marked"])] && client.markedString.length == 0 && !hasMarked);
    MSIMEApplyTransitionTrackingMarkedText(idle, client, MSIMEInlinePreeditStylePinyin, nil, &hasMarked);
    assert(client.events.count == 2);
    // A closing mark with no composition is still marked text, and must reach the client.
    MSIMEApplyTransitionTrackingMarkedText(idle, client, MSIMEInlinePreeditStyleEmpty, @"）", &hasMarked);
    assert([client.markedString isEqual:@"）"] && hasMarked);
    hasMarked = NO;
    // A commit keeps the clear after it, whatever the client was believed to hold.
    MSIMEApplyTransitionTrackingMarkedText(@{@"commit": @"你好", @"view": @{@"editing_text": @"", @"caret_position": @0}},
                                           client, MSIMEInlinePreeditStylePinyin, nil, &hasMarked);
    assert([client.events isEqual:(@[@"marked", @"marked", @"marked", @"commit", @"marked"])] && !hasMarked);
    // The empty inline style never marks anything, so it never needs to clear.
    MSIMEApplyTransitionTrackingMarkedText(@{@"view": @{@"editing_text": @"ni", @"caret_position": @2}}, client,
                                           MSIMEInlinePreeditStyleEmpty, nil, &hasMarked);
    assert(client.events.count == 5 && !hasMarked);
    // Without tracking nothing is known about the client, and the clear is always sent.
    MSIMEApplyTransitionWithPendingClosing(idle, client, MSIMEInlinePreeditStylePinyin, nil);
    assert(client.events.count == 6);
}

int main() {
    @autoreleasepool {
        TestTrackedMarkedText();
        FakeTextClient *client = [FakeTextClient new];
        client.events = [NSMutableArray array];
        TestEnginePreedit(client);
        TestEngineEdges(client);
        TestEngineMaintenance();
        TestCustomTranslationHTTPBridge();
        TestTencentTranslationHTTPBridge();
        // A pair the host owes the document: the closing mark is the tail of the marked text, so it
        // stays after the caret while the composition runs, and a commit takes it with it. IMK has
        // no caret setter, which is why the closing cannot simply be typed after the opening.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": NSNull.null, @"view": @{@"editing_text": @"ni", @"caret_position": @2}},
            client, MSIMEInlinePreeditStylePinyin, @"）");
        assert([client.markedString isEqual:@"ni）"] && client.selection.location == 2);
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": @"你好", @"view": @{@"editing_text": @"", @"caret_position": @0}}, client,
            MSIMEInlinePreeditStylePinyin, @"）");
        assert([client.committed isEqual:@"你好）"]);
        assert([client.markedString length] == 0);
        // Nothing pending is the ordinary case, and behaves as before.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": @"你好", @"view": @{@"editing_text": @"shi", @"caret_position": @1}}, client,
            MSIMEInlinePreeditStylePinyin, nil);
        assert([client.committed isEqual:@"你好"] && [client.markedString isEqual:@"shi"]);
        // An empty inline preedit still carries the mark, or the user would watch it disappear.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": NSNull.null, @"view": @{@"editing_text": @"ni", @"caret_position": @2}},
            client, MSIMEInlinePreeditStyleEmpty, @"】");
        assert([client.markedString isEqual:@"】"] && client.selection.location == 0);
        MSIMEApplyTransition(@{@"commit": @"你好", @"view": @{@"editing_text": @"shi", @"caret_position": @1}}, client);
        assert([client.committed isEqual:@"你好"]);
        assert([client.markedString isEqual:@"shi"] && client.selection.location == 1);
        MSIMEApplyTransition(@{@"commit": NSNull.null, @"view": @{@"editing_text": @"", @"caret_position": @5}}, client);
        assert([client.committed isEqual:@"你好"] && [client.markedString length] == 0 && client.selection.location == 0);
        // A Japanese composition shows the kana, which is what the user means and what Enter
        // commits. Showing the letters that were typed leaves the composition saying `nihon` while
        // the commit says にほん.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"nihon", @"reading": @"にほん",
                                          @"caret_position": @5}}, client);
        assert([client.markedString isEqual:@"にほん"] && client.selection.location == 3);
        // The same in the raw inline style: there is no romaji-versus-kana choice to make here,
        // the kana is the composition.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"view": @{@"editing_text": @"nihon", @"reading": @"にほん", @"caret_position": @5}},
            client, MSIMEInlinePreeditStyleRaw, nil);
        assert([client.markedString isEqual:@"にほん"]);
        // A caret the user moved into the middle of the letters keeps the letters on screen: the
        // Engine's offset is into the romaji and there is no map from it into the kana, so drawing
        // the kana here would put the caret somewhere it does not belong.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"nihon", @"reading": @"にほん",
                                          @"caret_position": @2}}, client);
        assert([client.markedString isEqual:@"nihon"] && client.selection.location == 2);
        // 笔画走同一条 reading 路径：editing_text 是键入的 hspnzx 字母，reading 与 preedit 是笔画字形，两种内嵌样式都画字形，光标落在末尾。
        for (NSNumber *style in @[@(MSIMEInlinePreeditStylePinyin), @(MSIMEInlinePreeditStyleRaw)]) {
            MSIMEApplyTransitionWithPendingClosing(
                @{@"view": @{@"scheme": @9, @"editing_text": @"hspx", @"preedit": @"一丨丿＊", @"reading": @"一丨丿＊", @"caret_position": @4}},
                client, (MSIMEInlinePreeditStyle)style.integerValue, nil);
            assert([client.markedString isEqual:@"一丨丿＊"] && client.selection.location == 4);
        }
        // Every other scheme is untouched: an empty reading is what they all carry.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"nihao", @"reading": @"",
                                          @"caret_position": @5}}, client);
        assert([client.markedString isEqual:@"nihao"]);
        // A phrase put together out of several selections: the piece already chosen leads the marked
        // text instead of going to the document, and the caret sits past it. The runtime hands it
        // over as its own field because caret_position counts into the editing text in this host's
        // string unit, and a prefix of Chinese characters is not the same length in both.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"paobu", @"phrase_prefix": @"海滩",
                                          @"caret_position": @5}}, client);
        assert([client.markedString isEqual:@"海滩paobu"] && client.selection.location == 7);
        // Two clauses, drawn differently: the chosen piece is settled and takes the thin underline,
        // the reading after it is still being worked on and takes the thick one, which is the macOS
        // convention and the same distinction the reference draws with its TSF display attributes.
        // Run together as one stretch of underlined text, nothing says where what the user already
        // chose ends.
        assert([client.marked isKindOfClass:NSAttributedString.class]);
        {
            NSAttributedString *clauses = client.marked;
            NSRange settled = NSMakeRange(0, 0);
            NSRange working = NSMakeRange(0, 0);
            NSDictionary *first = [clauses attributesAtIndex:0 effectiveRange:&settled];
            NSDictionary *rest = [clauses attributesAtIndex:2 effectiveRange:&working];
            assert(NSEqualRanges(settled, NSMakeRange(0, 2)));
            assert(NSEqualRanges(working, NSMakeRange(2, 5)));
            assert([first[NSUnderlineStyleAttributeName] isEqual:@(NSUnderlineStyleSingle)]);
            assert([rest[NSUnderlineStyleAttributeName] isEqual:@(NSUnderlineStyleThick)]);
            // Clause numbers, so a client that walks the segments sees two of them in order.
            assert([first[NSMarkedClauseSegmentAttributeName] isEqual:@0]);
            assert([rest[NSMarkedClauseSegmentAttributeName] isEqual:@1]);
        }
        // An ordinary composition with nothing chosen yet stays a plain string: there is only one
        // clause, and a host that never holds a phrase piece is unaffected by any of this.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"paobu", @"caret_position": @2}}, client);
        assert([client.marked isKindOfClass:NSString.class]);

        // The caret inside the remaining reading moves with it.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"paobu", @"phrase_prefix": @"海滩",
                                          @"caret_position": @2}}, client);
        assert([client.markedString isEqual:@"海滩paobu"] && client.selection.location == 4);
        // A client that asked for no inline preedit shows none of it, as the reference leaves its
        // own prefix length at zero for that style.
        MSIMEApplyTransitionWithPreeditStyle(@{@"view": @{@"editing_text": @"paobu",
                                                         @"phrase_prefix": @"海滩", @"caret_position": @5}},
                                             client, MSIMEInlinePreeditStyleEmpty);
        assert([client.markedString length] == 0);
        // A pair held open while a phrase is being assembled: both are tails of the same marked
        // text, and they are on opposite sides of the caret. The closing mark stays last so the
        // user can see what will be closed, and the chosen phrase piece stays first because it is
        // text that is already decided - the caret belongs between them, where typing continues.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": NSNull.null, @"view": @{@"editing_text": @"paobu", @"phrase_prefix": @"海滩",
                                                 @"caret_position": @5}},
            client, MSIMEInlinePreeditStylePinyin, @"）");
        assert([client.markedString isEqual:@"海滩paobu）"] && client.selection.location == 7);
        // Finishing the phrase closes the pair with it, and the whole phrase goes to the document
        // in one piece with the closing mark after it.
        MSIMEApplyTransitionWithPendingClosing(
            @{@"commit": @"海滩跑步", @"view": @{@"editing_text": @"", @"caret_position": @0}}, client,
            MSIMEInlinePreeditStylePinyin, @"）");
        assert([client.committed isEqual:@"海滩跑步）"] && [client.markedString length] == 0);

        // Finishing the phrase sends it out in one piece; the field is gone by then.
        MSIMEApplyTransition(@{@"commit": @"海滩跑步", @"view": @{@"editing_text": @"", @"caret_position": @0}},
                             client);
        assert([client.committed isEqual:@"海滩跑步"] && [client.markedString length] == 0);

        // Shuangpin full-pinyin display must not expose the raw key sequence.
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"b;", @"preedit": @"bing", @"caret_position": @2}}, client);
        assert([client.markedString isEqual:@"bing"] && client.selection.location == 4);
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"nihao", @"preedit": @"ni hao", @"caret_position": @2}}, client);
        assert([client.markedString isEqual:@"ni hao"] && client.selection.location == 2);
        // Every raw offset, including either side of an explicit apostrophe.
        for (NSArray *fixture in @[
            @[@"nihao", @"ni'hao", @[@0, @1, @2, @4, @5, @6]],
            @[@"nihao", @"ni hao", @[@0, @1, @2, @4, @5, @6]],
            @[@"ni'hao", @"ni hao", @[@0, @1, @2, @3, @4, @5, @6]],
            @[@"ni'hao", @"nihao", @[@0, @1, @2, @2, @3, @4, @5]],
            @[@"xian", @"xi'an", @[@0, @1, @2, @4, @5]],
            @[@"nihaoma", @"ni'hao'ma", @[@0, @1, @2, @4, @5, @6, @8, @9]],
            @[@"b;", @"bing", @[@4, @4, @4]],
            @[@"shnag", @"shang", @[@5, @5, @5, @5, @5, @5]],
            @[@"nihao", @"你hao", @[@4, @4, @4, @4, @4, @4]]]) {
            NSString *raw = fixture[0], *display = fixture[1];
            NSArray *offsets = fixture[2];
            assert(offsets.count == raw.length + 1);
            for (NSUInteger offset = 0; offset <= raw.length; ++offset) {
                MSIMEApplyTransition(@{@"view": @{@"editing_text": raw, @"preedit": display, @"caret_position": @(offset)}}, client);
                assert([client.markedString isEqual:display] && client.selection.location == [offsets[offset] unsignedIntegerValue]);
                assert(client.selection.length == 0);
            }
        }
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"shi", @"preedit": @"shi", @"caret_position": @1}}, client);
        assert([client.markedString isEqual:@"shi"] && client.selection.location == 1);
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"x", @"preedit": @"😀", @"caret_position": @1}}, client);
        assert([client.markedString isEqual:@"😀"] && client.selection.location == 2);
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"shi", @"preedit": NSNull.null, @"caret_position": NSNull.null}}, client);
        assert([client.markedString isEqual:@"shi"] && client.selection.location == 3);
        MSIMEApplyTransition(@{@"commit": @"合成", @"view": @{@"editing_text": @"", @"preedit": @"", @"caret_position": @0}}, client);
        assert([client.committed isEqual:@"合成"] && [client.markedString length] == 0 && client.selection.location == 0);
        assert(client.events.count >= 2 && [client.events[client.events.count - 2] isEqual:@"commit"] && [client.events.lastObject isEqual:@"marked"]);
        // The shared inline-preedit preference selects the actual marked text,
        // while preserving the display-specific caret contract.
        NSDictionary *styled = @{@"view": @{@"editing_text": @"b;", @"preedit": @"bing", @"caret_position": @1}};
        MSIMEApplyTransitionWithPreeditStyle(styled, client, MSIMEInlinePreeditStyleRaw);
        assert([client.markedString isEqual:@"b;"] && client.selection.location == 1);
        MSIMEApplyTransitionWithPreeditStyle(styled, client, MSIMEInlinePreeditStylePinyin);
        assert([client.markedString isEqual:@"bing"] && client.selection.location == 4);
        MSIMEApplyTransitionWithPreeditStyle(styled, client, MSIMEInlinePreeditStyleEmpty);
        assert([client.markedString length] == 0 && client.selection.location == 0);
        client.documentSelection = NSMakeRange(4, 0);
        client.following = @"】";
        assert([[MSIMETextClientFollowingCharacter(client) copy] isEqual:@"】"]);
        client.following = nil;
        assert(MSIMETextClientFollowingCharacter(client) == nil);
        client.document = @"a😀";
        client.documentSelection = NSMakeRange(client.document.length, 0);
        assert(MSIMETextClientPrecedingUnicodeScalar(client) == 0x1f600);
        client.documentSelection = NSMakeRange(1, 0);
        assert(MSIMETextClientPrecedingUnicodeScalar(client) == 'a');
        client.documentSelection = NSMakeRange(0, 0);
        assert(MSIMETextClientPrecedingUnicodeScalar(client) == 0);
    }
    return 0;
}
