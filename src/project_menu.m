#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <stdlib.h>
#include <string.h>

// AppKit and Rust access these only on the application main thread.
static NSMutableArray<NSNumber *> *requestedActions;
static NSMutableDictionary<NSNumber *, NSMenu *> *menus;
static NSMutableDictionary<NSNumber *, NSMutableArray<NSMenuItem *> *> *commands;
static NSInteger applicationInsertion;
static NSMutableDictionary<NSNumber *, NSNumber *> *availability;
static SEL editSelector(NSInteger code) {
    switch (code) {
        case 10: return @selector(cut:);
        case 11: return @selector(copy:);
        case 12: return @selector(paste:);
        case 13: return @selector(selectAll:);
        default: return NULL;
    }
}
static BOOL nativeTextEditing(void) {
    NSResponder *responder = NSApp.keyWindow.firstResponder;
    return [responder isKindOfClass:[NSTextView class]] || [responder isKindOfClass:[NSTextField class]];
}
@interface SymphosFileActions : NSObject <NSMenuItemValidation>
- (void)projectAction:(NSMenuItem *)sender;
@end
@implementation SymphosFileActions
- (BOOL)validateMenuItem:(NSMenuItem *)item {
    SEL edit = editSelector(item.tag);
    if (edit && nativeTextEditing()) return [NSApp targetForAction:edit] != nil;
    return availability[@(item.tag)].boolValue && !NSApp.modalWindow;
}
- (void)projectAction:(NSMenuItem *)sender {
    if (NSApp.currentEvent.type == NSEventTypeKeyDown && NSApp.currentEvent.isARepeat) return;
    SEL edit = editSelector(sender.tag);
    if (edit && nativeTextEditing()) { [NSApp sendAction:edit to:nil from:sender]; return; }
    if (![self validateMenuItem:sender]) return;
    [requestedActions addObject:@(sender.tag)];
}
@end
static SymphosFileActions *handler;
bool symphos_menu_begin(void) {
    if (!NSApp.mainMenu || !NSApp.mainMenu.itemArray.firstObject.submenu) return false;
    handler = [SymphosFileActions new];
    requestedActions = [NSMutableArray new];
    menus = [NSMutableDictionary new];
    commands = [NSMutableDictionary new];
    availability = [NSMutableDictionary new];
    menus[@(-1)] = NSApp.mainMenu;
    menus[@0] = NSApp.mainMenu.itemArray.firstObject.submenu;
    applicationInsertion = MIN((NSInteger)2, menus[@0].numberOfItems);
    return true;
}
static void appendItem(NSInteger parent, NSMenuItem *item) {
    if (parent == 0) [menus[@0] insertItem:item atIndex:applicationInsertion++];
    else [menus[@(parent)] addItem:item];
}
int symphos_menu_submenu(int parent, const char *title) {
    NSString *name = [NSString stringWithUTF8String:title];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:name];
    menu.autoenablesItems = YES;
    NSMenuItem *root = [[NSMenuItem alloc] initWithTitle:name action:nil keyEquivalent:@""];
    root.submenu = menu;
    appendItem(parent, root);
    int identifier = (int)menus.count - 1;
    menus[@(identifier)] = menu;
    if (parent == -1 && [name isEqualToString:@"Help"]) NSApp.helpMenu = menu;
    return identifier;
}
void symphos_menu_item(int parent, const char *title, int code, const char *key, unsigned modifiers) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:[NSString stringWithUTF8String:title]
        action:@selector(projectAction:) keyEquivalent:[NSString stringWithUTF8String:key]];
    item.target = handler;
    item.tag = code;
    item.keyEquivalentModifierMask = ((modifiers & 1) ? NSEventModifierFlagCommand : 0)
        | ((modifiers & 2) ? NSEventModifierFlagShift : 0)
        | ((modifiers & 4) ? NSEventModifierFlagOption : 0);
    // Availability comes from the live Rust workspace rather than responder validation.
    item.enabled = NO;
    if (!commands[@(code)]) commands[@(code)] = [NSMutableArray new];
    [commands[@(code)] addObject:item];
    appendItem(parent, item);
}
void symphos_menu_separator(int parent) { appendItem(parent, [NSMenuItem separatorItem]); }
void symphos_menu_finish(void) {
    [menus[@0] insertItem:[NSMenuItem separatorItem] atIndex:applicationInsertion];
    NSMenu *window = NSApp.windowsMenu;
    if (!window) {
        window = [[NSMenu alloc] initWithTitle:@"Window"];
        NSMenuItem *root = [[NSMenuItem alloc] initWithTitle:@"Window" action:nil keyEquivalent:@""];
        root.submenu = window;
        NSInteger helpIndex = [NSApp.mainMenu indexOfItemWithSubmenu:NSApp.helpMenu];
        [NSApp.mainMenu insertItem:root atIndex:helpIndex >= 0 ? helpIndex : NSApp.mainMenu.numberOfItems];
        NSApp.windowsMenu = window;
    }
    if (![window itemWithTitle:@"Minimize"]) {
        NSMenuItem *minimize = [[NSMenuItem alloc] initWithTitle:@"Minimize" action:@selector(performMiniaturize:) keyEquivalent:@"m"];
        minimize.keyEquivalentModifierMask = NSEventModifierFlagCommand;
        [window addItem:minimize];
        [window addItem:[[NSMenuItem alloc] initWithTitle:@"Zoom" action:@selector(performZoom:) keyEquivalent:@""]];
        [window addItem:[NSMenuItem separatorItem]];
        [window addItem:[[NSMenuItem alloc] initWithTitle:@"Bring All to Front" action:@selector(arrangeInFront:) keyEquivalent:@""]];
    }
}
void symphos_menu_state(int code, bool enabled, bool checked) {
    availability[@(code)] = @(enabled);
    for (NSMenuItem *item in commands[@(code)]) {
        if (item.enabled != enabled) item.enabled = enabled;
        NSControlStateValue state = checked ? NSControlStateValueOn : NSControlStateValueOff;
        if (item.state != state) item.state = state;
    }
}
int symphos_file_menu_take(void) {
    NSNumber *first = requestedActions.firstObject;
    if (!first) return 0;
    [requestedActions removeObjectAtIndex:0];
    return first.intValue;
}
// Called on the application's main thread; Rust owns and frees the returned UTF-8 path.
char *symphos_project_choose(int save, const char *initial) {
    NSSavePanel *panel = save ? [NSSavePanel savePanel] : [NSOpenPanel openPanel];
    panel.title = save ? @"Save Symphos Project" : @"Open Symphos Project";
    panel.allowedContentTypes = @[UTTypeJSON];
    panel.canCreateDirectories = YES;
    if (initial && initial[0]) {
        NSString *path = [NSString stringWithUTF8String:initial];
        panel.directoryURL = [NSURL fileURLWithPath:path.stringByDeletingLastPathComponent];
        if (save) panel.nameFieldStringValue = path.lastPathComponent;
    } else if (save) panel.nameFieldStringValue = @"Untitled.symphos.json";
    if (!save) {
        NSOpenPanel *open = (NSOpenPanel *)panel;
        open.canChooseDirectories = NO; open.canChooseFiles = YES; open.allowsMultipleSelection = NO;
    }
    if ([panel runModal] != NSModalResponseOK) return NULL;
    return strdup(panel.URL.path.UTF8String);
}
void symphos_project_path_free(char *path) { free(path); }
