#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdint.h>
#include <math.h>

// AppKit owns the onscreen loupe and consumes the sampling click/Escape.
// All access, including the completion handler, stays on the main thread.
static NSColorSampler *sampler;
static uint64_t generation;
static int result;
static uint8_t sampled[3];

uint64_t symphos_color_sample_start(void) {
    if (![NSThread isMainThread] || !NSApp || sampler) return 0;
    uint64_t request = ++generation;
    result = 0;
    sampler = [NSColorSampler new];
    [sampler showSamplerWithSelectionHandler:^(NSColor *color) {
        NSColor *srgb = [color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        if (srgb) {
            sampled[0] = (uint8_t)lround(fmin(1.0, fmax(0.0, srgb.redComponent)) * 255.0);
            sampled[1] = (uint8_t)lround(fmin(1.0, fmax(0.0, srgb.greenComponent)) * 255.0);
            sampled[2] = (uint8_t)lround(fmin(1.0, fmax(0.0, srgb.blueComponent)) * 255.0);
            result = 1;
        } else {
            result = 2;
        }
        sampler = nil;
    }];
    return request;
}

int symphos_color_sample_poll(uint64_t request, uint8_t *rgb) {
    if (![NSThread isMainThread] || request != generation) return 2;
    if (result == 1) {
        for (int i = 0; i < 3; ++i) rgb[i] = sampled[i];
    }
    return result;
}
