#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

static _Atomic int requestedAction = 0;
@interface SymphosFileActions : NSObject
- (void)projectAction:(NSMenuItem *)sender;
@end
@implementation SymphosFileActions
- (void)projectAction:(NSMenuItem *)sender { atomic_store(&requestedAction, (int)sender.tag); }
@end
static SymphosFileActions *handler;
void symphos_file_menu_install(void) {
    NSMenu *menu = NSApp.mainMenu;
    if (!menu) return;
    if (!handler) handler = [SymphosFileActions new];
    NSMenu *application = menu.itemArray.firstObject.submenu;
    if (application && ![application itemWithTag:5]) {
        NSMenuItem *settings = [[NSMenuItem alloc] initWithTitle:@"Settings…" action:@selector(projectAction:) keyEquivalent:@","];
        settings.target = handler; settings.tag = 5;
        settings.keyEquivalentModifierMask = NSEventModifierFlagCommand;
        NSInteger index = MIN((NSInteger)2, application.numberOfItems);
        [application insertItem:settings atIndex:index];
        [application insertItem:[NSMenuItem separatorItem] atIndex:index + 1];
    }
    if ([menu itemWithTitle:@"File"]) return;
    NSMenu *file = [[NSMenu alloc] initWithTitle:@"File"];
    NSArray *titles = @[@"New Project", @"Open Project…", @"Save Project", @"Save Project As…"];
    NSArray *keys = @[@"n", @"o", @"s", @"s"];
    for (NSUInteger i = 0; i < titles.count; ++i) {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:titles[i] action:@selector(projectAction:) keyEquivalent:keys[i]];
        item.target = handler; item.tag = i == 0 ? 4 : (NSInteger)i;
        item.keyEquivalentModifierMask = NSEventModifierFlagCommand | (i == 3 ? NSEventModifierFlagShift : 0);
        [file addItem:item];
    }
    NSMenuItem *root = [[NSMenuItem alloc] initWithTitle:@"File" action:nil keyEquivalent:@""];
    root.submenu = file;
    [menu insertItem:root atIndex:MIN((NSInteger)1, menu.numberOfItems)];
}
int symphos_file_menu_take(void) { return atomic_exchange(&requestedAction, 0); }
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
