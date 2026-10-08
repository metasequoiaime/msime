#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

typedef NSDictionary *_Nullable (^MSIMESnapshotNextRecord)(NSError *_Nullable *error);

/// Foundation adapter for macOS input controllers and iOS keyboard extensions.
/// Construct and use on the main thread. No Tauri process is required.
@interface MSIMEClientSession : NSObject
typedef void (^MSIMEVoiceProviderUpdate)(NSString *text, BOOL final);
typedef void (^MSIMEVoiceProviderPhase)(NSUInteger phase);
FOUNDATION_EXPORT NSNotificationName const MSIMEClientSessionDidReplaceSnapshotNotification;
/// The validated creation options, copied for native maintenance UI; never mutable by callers.
@property(nonatomic, readonly) NSDictionary<NSString *, id> *hostOptions;
- (nullable instancetype)initWithOptions:(NSDictionary<NSString *, id> *)options error:(NSError **)error;
- (instancetype)init NS_UNAVAILABLE;
- (nullable NSDictionary<NSString *, id> *)setFocused:(BOOL)focused error:(NSError **)error;
/// Returns the current View, not a transition; preserves live composition.
- (nullable NSDictionary<NSString *, id> *)setChinesePunctuationEnabled:(BOOL)enabled error:(NSError **)error;
/// Returns a View; updates Engine paired-punctuation behavior without persisting preferences.
- (nullable NSDictionary<NSString *, id> *)setPairedPunctuationEnabled:(BOOL)enabled error:(NSError **)error;
/// Tell the Engine a pair this host closed on its own is finished.
///
/// Book title marks nest: the Engine counts how many 《 are open so that one typed inside another
/// comes out 〈. A host that supplies 》 itself never sends the `>` that would unwind that count, so
/// without this the next 《》 the user types degrades into 〈〉. The reference calls its own
/// `BalanceNestPairAfterAutoClose` at the same point and for the same reason.
- (BOOL)balancePairedPunctuationAfterAutoClose:(uint8_t)opening error:(NSError **)error;
/// Returns a View; lock is follow, chinese, or english and maps to the Engine's 0/1/2 values.
- (nullable NSDictionary<NSString *, id> *)setPunctuationLock:(NSString *)lock error:(NSError **)error;
/// Returns a View (not a transition). Finish composition before changing mode.
- (nullable NSDictionary *)setDedicatedEnglishEnabled:(BOOL)enabled error:(NSError **)error;
/// Compatibility selector with the same session-mode restoration behavior.
- (nullable NSDictionary *)setEnglishMode:(BOOL)enabled error:(NSError **)error;
/// Returns a View; remembers an explicit live width override across recreation.
- (nullable NSDictionary *)setCharacterWidthFull:(BOOL)fullwidth error:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)typeASCII:(uint8_t)character shift:(BOOL)shift error:(NSError **)error;
/// Finish the highlighted composition and append a literal ASCII punctuation mark.
- (nullable NSDictionary<NSString *, id> *)punctuationASCII:(uint8_t)ascii error:(NSError **)error;
/// Apply the Engine's punctuation policy to an ASCII punctuation mark.
- (nullable NSDictionary<NSString *, id> *)punctuation:(uint8_t)ascii error:(NSError **)error;
/// Resolve smart punctuation using the host's preceding Unicode scalar.
- (nullable NSDictionary<NSString *, id> *)punctuation:(uint8_t)ascii preceding:(uint32_t)preceding error:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)command:(uint32_t)command error:(NSError **)error;
/// Clears this session's Engine candidate cache and returns the refreshed transition.
- (nullable NSDictionary<NSString *, id> *)resetCacheWithError:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)selectGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error;
- (nullable NSDictionary *)selectEdgeGeneration:(uint64_t)generation index:(NSUInteger)index edge:(uint8_t)edge error:(NSError **)error;
- (nullable NSDictionary *)pinGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error;
- (nullable NSDictionary *)removeGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error;
- (nullable NSDictionary *)fixGeneration:(uint64_t)generation index:(NSUInteger)index position:(uint8_t)position error:(NSError **)error;
- (nullable NSDictionary *)clearPositionGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)viewWithError:(NSError **)error;
/// Re-rank the visible candidates with the settled model, after the host's typing pause.
///
/// The host owns the clock: only it knows whether a keystroke arrived while the pass ran. Answers
/// {moved, view}; `moved` is NO when the order did not change, which is the signal to leave the
/// candidate window alone rather than repaint it identically. Inert, and immediately NO, when no
/// settled model is installed. No network I/O.
- (nullable NSDictionary *)rerankSettledWithError:(NSError **)error;
/// Copied Engine query, or nil when ineligible. Does not perform network I/O.
- (nullable NSDictionary *)onlineQueryWithError:(NSError **)error;
/// Build a descriptor for the copied query using this session's current AI
/// credentials. The shared host validates query/config identity; never log it.
- (nullable NSDictionary *)aiRequestForQuery:(NSDictionary *)query error:(NSError **)error;
+ (nullable NSString *)cloudRequestURLForQuery:(NSDictionary *)query error:(NSError **)error;
/// Shared bounded parser and stale-query guard; returns {applied,view}.
- (nullable NSDictionary *)applyCloudResponse:(NSData *)body query:(NSDictionary *)query error:(NSError **)error;
/// Apply a copied cloud (source=0) or AI (source=1) string batch on the session thread.
/// Engine validates candidate limit, eligibility and query identity. No network I/O.
- (nullable NSDictionary *)applyOnlineCandidates:(NSArray<NSString *> *)candidates source:(NSUInteger)source
                                           query:(NSDictionary *)query error:(NSError **)error;
/// Copied enabled translation query, or nil when no candidates are eligible.
/// May contain custom/Tencent credentials for native transport; never log it.
- (nullable NSDictionary *)translationQueryWithError:(NSError **)error;
/// Pure descriptor construction. Contains optional credentials; never log it.
+ (nullable NSDictionary *)customTranslationHTTPRequest:(NSDictionary *)request error:(NSError **)error;
/// Signed Tencent descriptor. Send body_utf8 unchanged; never log credentials.
+ (nullable NSDictionary *)tencentTranslationHTTPRequest:(NSDictionary *)request error:(NSError **)error;
/// Signed NiuTrans v2 form descriptor. Send body_utf8 unchanged; never log it.
+ (nullable NSDictionary *)niuTransTranslationHTTPRequest:(NSDictionary *)request error:(NSError **)error;
+ (nullable NSString *)parseNiuTransTranslationResponse:(NSData *)body error:(NSError **)error;
/// A gloss this host produced itself, formatted as provider replies are: whitespace collapsed, ends trimmed. nil when nothing usable is left.
+ (nullable NSString *)formatTranslationGloss:(NSString *)gloss error:(NSError **)error;
/// Pure AI descriptor with credentials; never log it or follow HTTP redirects.
+ (nullable NSDictionary *)aiHTTPRequest:(NSDictionary *)request error:(NSError **)error;
+ (nullable NSArray<NSString *> *)parseAIResponse:(NSData *)body limit:(NSUInteger)limit error:(NSError **)error;
/// Worker-only private disk I/O; same bounded request as msime_client_learned_translation_request.
/// Supply a private user directory, never packaged resources. Do not log learned text.
+ (nullable NSDictionary *)learnedTranslationRequest:(NSDictionary *)request error:(NSError **)error;
/// Exact batch positions: NSString or NSNull. nil means an invalid response.
+ (nullable NSArray *)parseTencentTranslationResponse:(NSData *)body expectedCount:(NSUInteger)count error:(NSError **)error;
/// Pure script/direction filtering for {target_language,candidates:[{text,source}]}.
+ (nullable NSArray<NSDictionary *> *)customTranslationPlan:(NSDictionary *)request error:(NSError **)error;
/// Parse only a successful HTTP response; nil without error means no usable translation.
+ (nullable NSString *)parseCustomTranslationResponse:(NSData *)body error:(NSError **)error;
/// Whether a custom (DeepLX-compatible) or NiuTrans reply reports a failure - malformed, a non-200 code, errorCode/errorMsg - rather than an answer. The parsers return nil for both a failure and an answer with no text; only answers may be negative-cached.
+ (BOOL)customTranslationReplyFailed:(NSData *)body;
+ (BOOL)niuTransTranslationReplyFailed:(NSData *)body;
/// Offline dictionary lookup; may run on a worker with copied {generation,candidates:[{text,source}]}.
+ (nullable NSDictionary *)candidateGlossRequest:(NSDictionary *)request resources:(NSString *)resources error:(NSError **)error;
// {generation,items:[{text,language:"en"}]} -> {generation,pronunciations:[{text,language,pronunciation}]}; owns no session, for a worker queue.
+ (nullable NSDictionary *)pronunciationRequest:(NSDictionary *)request resources:(NSString *)resources error:(NSError **)error;
// {generation,texts:[...]} -> {generation,breakdowns:[{text,breakdown}]}: "我 I · 喜欢 to like · 你 you"; owns no session, for a worker queue.
+ (nullable NSDictionary *)glossBreakdownRequest:(NSDictionary *)request resources:(NSString *)resources error:(NSError **)error;
/// Apply on the originating session/thread only. A stale generation is ignored.
- (nullable NSDictionary *)applyTranslations:(NSArray<NSDictionary *> *)translations generation:(uint64_t)generation error:(NSError **)error;
/// Effect sounds and background music from this session's preferences.plugins, played by the host library on macOS (see msime_client_key_sound). These sit on the key path: they only queue a request, never block or read files, and answer whether one was queued. NO when nothing is switched on, the session is closed, or the platform does not play (iOS). Never call them for a key typed into a secure field.
/// keyClass is 0 for any other key, 1 space, 2 enter, 3 backspace.
- (BOOL)keySound:(uint32_t)keyClass;
/// Call when a transition commits text; plays the commit sample or advances a melody that follows commits.
- (BOOL)commitSound;
/// YES while the input method is active in a field that is not secure; music plays only in between.
- (BOOL)setMusicActive:(BOOL)active;
/// The typing effect of one key or commit to draw (msime_client_typing_effect): event 0-3 the key class, 4 a commit, 5 a backspace by another route, with 0x100 for an auto-repeat and 0x200 to keep the tier-up sound quiet. Answers the packed combo count, tier-up bit and style; 0 when effects and the combo counter are off or the session is closed. Also for the key path; never for a key typed into a secure field.
- (uint32_t)typingEffect:(uint32_t)event;
/// The session's resolved typing effect (msime_client_typing_effect_settings): {pack, issue, style, intensity, colors, duration_ms, particles, combo_counter}. It may read the selected effect pack's manifest, so call it after a preference update and after a focus-in, never per key.
- (nullable NSDictionary<NSString *, id> *)typingEffectSettingsWithError:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)setCandidatePageSize:(uint8_t)size error:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)updatePreferencesSnapshot:(NSDictionary<NSString *, id> *)snapshot error:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)startVoiceWithError:(NSError **)error;
- (BOOL)cancelVoiceWithError:(NSError **)error;
- (nullable NSDictionary<NSString *, id> *)applyVoiceText:(NSString *)text generation:(uint64_t)generation error:(NSError **)error;
/// User-owned provider socket boundary for cloud ASR; no credentials are logged.
- (nullable NSDictionary<NSString *, id> *)voiceProviderRequest:(NSDictionary *)query socket:(NSString *)socket error:(NSError **)error;
- (BOOL)voiceProviderStream:(NSDictionary *)query socket:(NSString *)socket update:(MSIMEVoiceProviderUpdate)update phase:(MSIMEVoiceProviderPhase)phase error:(NSError **)error;
- (BOOL)voiceProviderCancelSocket:(NSString *)socket generation:(uint64_t)generation error:(NSError **)error;
- (BOOL)voiceProviderStopSocket:(NSString *)socket generation:(uint64_t)generation error:(NSError **)error;
/// Decode a binary Doubao response frame without retaining frame bytes.
+ (nullable NSDictionary<NSString *, id> *)doubaoDecodeFrame:(NSData *)frame error:(NSError **)error;
+ (nullable NSData *)doubaoStartFrameEnableITN:(BOOL)enableITN
                                  punctuation:(BOOL)enablePunctuation
                                          DDC:(BOOL)enableDDC
                               boostingTable:(NSString *)boostingTable
                                       error:(NSError **)error;
+ (nullable NSData *)doubaoAudioFrameWithSequence:(int32_t)sequence
                                              PCM:(NSData *)pcm
                                            final:(BOOL)finalChunk
                                            error:(NSError **)error;
/// Management is separate from live sessions; call only after all sessions are closed.
+ (nullable NSDictionary<NSString *, id> *)dictionaryRequest:(NSDictionary<NSString *, id> *)request error:(NSError **)error;
+ (nullable NSDictionary<NSString *, id> *)handwritingProviderRequest:(NSDictionary<NSString *, id> *)request error:(NSError **)error;
+ (NSDictionary<NSString *, id> *)handwritingProviderRequest:(NSDictionary<NSString *, id> *)request;
+ (NSDictionary<NSString *, id> *)emojiCatalogRequest:(NSDictionary<NSString *, id> *)request;
+ (NSDictionary<NSString *, id> *)clipboardHistoryRequest:(NSString *)directory;
+ (NSDictionary<NSString *, id> *)enableClipboardHistoryRequest:(NSString *)directory;
+ (NSDictionary<NSString *, id> *)clipboardCaptureEnabledRequest:(NSString *)directory;
+ (NSDictionary<NSString *, id> *)removeClipboardHistoryRequest:(NSDictionary<NSString *, id> *)request;
+ (NSDictionary<NSString *, id> *)captureClipboardHistoryRequest:(NSDictionary<NSString *, id> *)request;
/// Return the current local dictionary version without exposing dictionary text.
+ (nullable NSString *)snapshotVersionForOptions:(NSDictionary<NSString *, id> *)options error:(NSError **)error;
/// Dynamic Swift-backend form; returns {version} or {error}.
+ (NSDictionary<NSString *, id> *)snapshotVersion:(NSDictionary<NSString *, id> *)options;
/// Discard a process-owned, unpublished preparation handle.
+ (BOOL)discardSnapshotHandle:(uint64_t)handle error:(NSError **)error;
/// Dynamic Swift-backend form; returns {discarded} or {error}.
+ (NSDictionary<NSString *, id> *)discardSnapshot:(NSDictionary<NSString *, id> *)parameters;
/// Atomically publish and recreate the active session; must be called on main thread while idle.
+ (BOOL)applySnapshotHandle:(uint64_t)handle expectedVersion:(NSString *)version error:(NSError **)error;
/// Dynamic Swift-backend form; parameters contains handle and expectedVersion.
+ (NSDictionary<NSString *, id> *)applySnapshot:(NSDictionary<NSString *, id> *)parameters;
/// Current validated host options for the live input session, or an error dictionary.
+ (nullable NSDictionary<NSString *, id> *)activeHostOptions;
/// Return whether the active session is idle enough for dictionary activation.
/// The check is main-thread-only and never finishes a user's composition.
+ (NSDictionary<NSString *, id> *)snapshotActivationReady;
/// Prepare a bounded, checksummed record stream synchronously; invoke off-main-thread.
+ (nullable NSDictionary<NSString *, id> *)prepareSnapshotRequest:(NSDictionary<NSString *, id> *)request
                                                       nextRecord:(MSIMESnapshotNextRecord)nextRecord
                                                            error:(NSError **)error;
/// Dynamic Swift-backend form; parameters contains request and nextRecord.
+ (NSDictionary<NSString *, id> *)prepareSnapshot:(NSDictionary<NSString *, id> *)parameters;
/// Prepare isolated Engine working data; call off the main thread and before creating sessions.
+ (nullable NSDictionary<NSString *, id> *)prepareHostWithResourcesDirectory:(NSString *)resourcesDirectory stateRoot:(NSString *)stateRoot error:(NSError **)error;
+ (nullable NSDictionary<NSString *, id> *)savePreferencesInDirectory:(NSString *)directory expectedRevision:(uint64_t)revision snapshot:(NSDictionary<NSString *, id> *)snapshot error:(NSError **)error;
/// Read the complete shared snapshot, including its current revision.
+ (nullable NSDictionary<NSString *, id> *)loadPreferencesInDirectory:(NSString *)directory error:(NSError **)error;
/// Repair a preferences document that is not well-formed JSON after backing it up beside itself; see msime_client_recover_preferences. Returns {recovered, snapshot, backup_path?, backup_name?, salvaged?}. Blocks on disk: call off the main thread.
+ (nullable NSDictionary<NSString *, id> *)recoverPreferencesInDirectory:(NSString *)directory error:(NSError **)error;
/// Start on main thread; disk/lock work runs in background, completion on main.
- (void)reloadPreferencesDirectory:(NSString *)directory completion:(void (^)(NSDictionary * _Nullable result, NSError * _Nullable error))completion;
- (BOOL)closeWithError:(NSError **)error;
@end

NS_ASSUME_NONNULL_END
