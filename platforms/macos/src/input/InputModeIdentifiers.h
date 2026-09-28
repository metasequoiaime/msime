#pragma once
#import <Foundation/Foundation.h>

// The three input modes Info.plist.in declares. The menu bar shows the active mode's icon, 中, 英 or 日, which is how macOS carries the persistent input-mode indicator the Windows tray's language-bar icon provides. info-plist-names checks these literals against the plist.
static NSString *const MSIMEChineseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Hans";
static NSString *const MSIMEEnglishInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Roman";
static NSString *const MSIMEJapaneseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Japanese";

// English mode wins over the scheme: 英 is shown whenever the controller passes keys through, whatever scheme is behind it. Otherwise the japanese scheme shows 日 and every Chinese scheme shows 中.
static inline NSString *MSIMEInputModeIDFor(BOOL english, BOOL japanese) {
    return english ? MSIMEEnglishInputModeID : japanese ? MSIMEJapaneseInputModeID : MSIMEChineseInputModeID;
}

static inline BOOL MSIMEIsInputModeID(id value) {
    return [value isKindOfClass:NSString.class] &&
           ([value isEqualToString:MSIMEChineseInputModeID] || [value isEqualToString:MSIMEEnglishInputModeID] ||
            [value isEqualToString:MSIMEJapaneseInputModeID]);
}

static inline BOOL MSIMEEnglishForInputModeID(NSString *identifier) {
    return [identifier isEqualToString:MSIMEEnglishInputModeID];
}

static inline BOOL MSIMEJapaneseForInputModeID(NSString *identifier) {
    return [identifier isEqualToString:MSIMEJapaneseInputModeID];
}

// Keeps the system's selected input mode and the controller's Chinese/English/Japanese state in step without either side echoing the other. The selected mode is global to the login session, so one state serves every controller instance.
//
// `current` is the mode last reported by the system or last requested by the controller; a report that repeats it is the system confirming what is already shown, not a user choice, so it does not flip the controller's state. `selecting` is set while the controller is asking the client to switch, so a report delivered synchronously from inside that call is not treated as a new choice either.
struct MSIMESystemInputModeState {
    NSString *current = nil;
    bool selecting = false;
};

// The system's selected input mode is global to the login session, so every controller instance shares one record of it. Leaving the input method clears it, so the first report after coming back is adopted even if it names the mode shown before leaving.
inline MSIMESystemInputModeState &MSIMESharedSystemInputModeState() {
    static MSIMESystemInputModeState state;
    return state;
}

static inline void MSIMEResetSystemInputModeState(MSIMESystemInputModeState &state) { state = MSIMESystemInputModeState{}; }

// Records a mode the system reported through setValue:forTag:client:. Returns YES when the controller should adopt it: a known mode that differs from the one already shown and that the controller did not just request itself.
static inline BOOL MSIMEAdoptReportedInputMode(MSIMESystemInputModeState &state, id value) {
    if (!MSIMEIsInputModeID(value)) return NO;
    const BOOL changed = ![value isEqualToString:state.current];
    state.current = [value copy];
    return changed && !state.selecting;
}

// Whether the system offers a mode for selection. A mode the user removed in System Settings, or one an install from before the English or Japanese mode existed has not registered yet, cannot be selected, and asking for it would leave `current` naming a mode the menu bar does not show - the next report of the real one would then flip the controller's state back.
using MSIMEInputModeAvailability = BOOL (*)(NSString *identifier);

// Asks the client to show `mode`, unless it already does or the system does not offer it. Returns whether a switch was requested. The japanese scheme falls back to 中 when the Japanese mode is not offered: the controller is still composing through the Engine, and 中 is closer to that than an 英 left over from before.
static inline BOOL MSIMESelectSystemInputMode(MSIMESystemInputModeState &state, NSString *mode, id client,
                                              MSIMEInputModeAvailability available) {
    if (!available) return NO;
    if ([mode isEqualToString:MSIMEJapaneseInputModeID] && !available(mode)) mode = MSIMEChineseInputModeID;
    if (state.selecting || [mode isEqualToString:state.current] || ![client respondsToSelector:@selector(selectInputMode:)] ||
        !available(mode))
        return NO;
    state.current = mode;
    state.selecting = true;
    [client performSelector:@selector(selectInputMode:) withObject:mode];
    state.selecting = false;
    return YES;
}
