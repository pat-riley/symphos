#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdint.h>

void symphos_window_chrome(uint8_t red, uint8_t green, uint8_t blue, bool dark) {
    if (![NSThread isMainThread] || !NSApp) return;
    NSWindow *window = NSApp.mainWindow;
    if (!window || [window isKindOfClass:[NSPanel class]]) return;

    static __weak NSWindow *styledWindow;
    static uint32_t styledPalette;
    const uint32_t palette = ((uint32_t)red << 24) | ((uint32_t)green << 16)
        | ((uint32_t)blue << 8) | dark;
    if (styledWindow == window && styledPalette == palette) return;

    // An opaque, theme-colored backing prevents desktop tint from changing the
    // title bar. Keep native title text, traffic lights and window dragging.
    window.appearance = [NSAppearance appearanceNamed:dark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
    window.backgroundColor = [NSColor colorWithSRGBRed:red / 255.0
                                               green:green / 255.0
                                                blue:blue / 255.0 alpha:1.0];
    window.titlebarAppearsTransparent = YES;
    window.titlebarSeparatorStyle = NSTitlebarSeparatorStyleNone;
    styledWindow = window;
    styledPalette = palette;
}
