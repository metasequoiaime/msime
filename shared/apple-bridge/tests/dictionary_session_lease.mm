#include "DictionarySessionLease.h"
#include <cassert>
#include <memory>
#include <stdexcept>

// The lease is what decides whether a dictionary snapshot may be published. Every live keyboard
// bridge holds a shared lease, publication requires the only remaining one, and the upgrade from
// shared to exclusive is not atomic -- so a publication that throws has to leave the lease back
// in the shared state it started from, or the session that survived the failure can never publish
// again. These are the cases that distinguish those states; the bridge sources are shared with
// the Apple client unchanged, so the behaviour asserted here is that client's behaviour.
int main() {
    @autoreleasepool {
        using metasequoia::apple::DictionarySessionLease;
        NSURL *symlinkRoot = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                       stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        NSURL *outside = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                   stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([NSFileManager.defaultManager createDirectoryAtURL:symlinkRoot
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        assert([NSFileManager.defaultManager createDirectoryAtURL:outside
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        NSURL *outsideLock = [outside URLByAppendingPathComponent:@"outside.lock"];
        assert([@"synthetic-lock-target" writeToURL:outsideLock atomically:YES encoding:NSUTF8StringEncoding error:nil]);
        NSURL *linkedSessions = [symlinkRoot URLByAppendingPathComponent:@"dictionary-sessions.lock"];
        assert([NSFileManager.defaultManager createSymbolicLinkAtURL:linkedSessions
                                               withDestinationURL:outsideLock
                                                              error:nil]);
        bool rejected = false;
        try {
            DictionarySessionLease rejectedLease(symlinkRoot);
        } catch (const std::exception &) {
            rejected = true;
        }
        assert(rejected);
        NSString *unchanged = [NSString stringWithContentsOfURL:outsideLock
                                                          encoding:NSUTF8StringEncoding
                                                             error:nil];
        assert([unchanged isEqualToString:@"synthetic-lock-target"]);
        assert([NSFileManager.defaultManager removeItemAtURL:symlinkRoot error:nil]);
        assert([NSFileManager.defaultManager removeItemAtURL:outside error:nil]);

        NSURL *linkedContainer = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                          stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        NSURL *linkedOutside = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                        stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([NSFileManager.defaultManager createDirectoryAtURL:linkedContainer
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        assert([NSFileManager.defaultManager createDirectoryAtURL:linkedOutside
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        NSURL *linkedAlias = [linkedContainer URLByAppendingPathComponent:@"linked"];
        assert([NSFileManager.defaultManager createSymbolicLinkAtURL:linkedAlias
                                               withDestinationURL:linkedOutside
                                                              error:nil]);
        NSURL *linkedRoot = [linkedAlias URLByAppendingPathComponent:@"session"];
        rejected = false;
        try {
            DictionarySessionLease rejectedLease(linkedRoot);
        } catch (const std::exception &) {
            rejected = true;
        }
        assert(rejected);
        assert(![NSFileManager.defaultManager fileExistsAtPath:
            [linkedOutside URLByAppendingPathComponent:@"dictionary-sessions.lock"].path]);
        assert(![NSFileManager.defaultManager fileExistsAtPath:
            [linkedOutside URLByAppendingPathComponent:@"dictionary-publication.lock"].path]);
        assert(![NSFileManager.defaultManager fileExistsAtPath:
            [linkedOutside URLByAppendingPathComponent:@"session"].path]);
        assert([NSFileManager.defaultManager removeItemAtURL:linkedContainer error:nil]);
        assert([NSFileManager.defaultManager removeItemAtURL:linkedOutside error:nil]);

        NSURL *gateRoot = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                   stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        NSURL *gateOutside = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                       stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([NSFileManager.defaultManager createDirectoryAtURL:gateRoot
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        assert([NSFileManager.defaultManager createDirectoryAtURL:gateOutside
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        NSURL *outsideGate = [gateOutside URLByAppendingPathComponent:@"outside.lock"];
        assert([@"synthetic-gate-target" writeToURL:outsideGate atomically:YES encoding:NSUTF8StringEncoding error:nil]);
        NSURL *linkedGate = [gateRoot URLByAppendingPathComponent:@"dictionary-publication.lock"];
        assert([NSFileManager.defaultManager createSymbolicLinkAtURL:linkedGate
                                               withDestinationURL:outsideGate
                                                              error:nil]);
        rejected = false;
        try {
            DictionarySessionLease rejectedLease(gateRoot);
        } catch (const std::exception &) {
            rejected = true;
        }
        assert(rejected);
        unchanged = [NSString stringWithContentsOfURL:outsideGate
                                               encoding:NSUTF8StringEncoding
                                                  error:nil];
        assert([unchanged isEqualToString:@"synthetic-gate-target"]);
        assert([NSFileManager.defaultManager removeItemAtURL:gateRoot error:nil]);
        assert([NSFileManager.defaultManager removeItemAtURL:gateOutside error:nil]);

        NSURL *hardlinkRoot = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                       stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        NSURL *hardlinkOutside = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                          stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([NSFileManager.defaultManager createDirectoryAtURL:hardlinkRoot
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        assert([NSFileManager.defaultManager createDirectoryAtURL:hardlinkOutside
                                         withIntermediateDirectories:YES
                                                          attributes:nil
                                                               error:nil]);
        NSURL *outsideHardlink = [hardlinkOutside URLByAppendingPathComponent:@"outside.lock"];
        assert([@"synthetic-hardlink-target" writeToURL:outsideHardlink atomically:YES encoding:NSUTF8StringEncoding error:nil]);
        NSURL *linkedSessionsFile = [hardlinkRoot URLByAppendingPathComponent:@"dictionary-sessions.lock"];
        assert([NSFileManager.defaultManager linkItemAtURL:outsideHardlink
                                                      toURL:linkedSessionsFile
                                                      error:nil]);
        rejected = false;
        try {
            DictionarySessionLease rejectedLease(hardlinkRoot);
        } catch (const std::exception &) {
            rejected = true;
        }
        assert(rejected);
        unchanged = [NSString stringWithContentsOfURL:outsideHardlink
                                               encoding:NSUTF8StringEncoding
                                                  error:nil];
        assert([unchanged isEqualToString:@"synthetic-hardlink-target"]);
        assert([NSFileManager.defaultManager removeItemAtURL:hardlinkRoot error:nil]);
        assert([NSFileManager.defaultManager removeItemAtURL:hardlinkOutside error:nil]);

        NSURL *root = [NSURL fileURLWithPath:[NSTemporaryDirectory()
                                                 stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        {
            DictionarySessionLease first(root);
            auto other = std::make_unique<DictionarySessionLease>(root);
            bool ran = false;
            // A second live session blocks publication from either side.
            assert(!first.exclusively([&] { ran = true; }));
            assert(!ran);
            assert(!other->exclusively([&] { ran = true; }));
            assert(!ran);

            other.reset();
            bool failed = false;
            try {
                first.exclusively([] { throw std::runtime_error("synthetic publication failure"); });
            } catch (const std::exception &) {
                failed = true;
            }
            // The operation's exception reaches the caller rather than being swallowed into a
            // "publication declined" result: a failed publication is not a busy lease.
            assert(failed);

            // Having restored sharing, a newly created session must again be able to block.
            other = std::make_unique<DictionarySessionLease>(root);
            assert(!other->exclusively([&] { ran = true; }));
            assert(!ran);

            other.reset();
            assert(first.exclusively([&] { ran = true; }));
            assert(ran);
        }
        assert([NSFileManager.defaultManager removeItemAtURL:root error:nil]);
    }
    return 0;
}
