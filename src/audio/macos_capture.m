// Core Audio ownership stays here; the realtime path only inspects buffers and
// calls the Rust SPSC writer. No Objective-C messages or allocations in io_proc.
#import <CoreAudio/CoreAudio.h>
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>
#import <Foundation/Foundation.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// Pointers are borrowed only for the duration of the callback. Strides are in floats.
typedef void (*SampleCallback)(void *, const float *, const float *, uint32_t,
                               uint32_t, uint32_t);
typedef struct {
    AudioObjectID tap;
    AudioObjectID device;
    AudioDeviceIOProcID io;
    SampleCallback callback;
    void *context;
    AudioStreamBasicDescription format;
    atomic_bool changed;
    bool tap_listener;
    bool output_listener;
    bool started;
} Capture;

static AudioObjectPropertyAddress address(AudioObjectPropertySelector selector) {
    return (AudioObjectPropertyAddress){selector, kAudioObjectPropertyScopeGlobal,
                                       kAudioObjectPropertyElementMain};
}

static OSStatus changed(AudioObjectID object, UInt32 count,
                        const AudioObjectPropertyAddress *properties, void *context) {
    (void)object; (void)count; (void)properties;
    atomic_store_explicit(&((Capture *)context)->changed, true, memory_order_release);
    return noErr;
}

static OSStatus io_proc(AudioObjectID device, const AudioTimeStamp *now,
                        const AudioBufferList *input, const AudioTimeStamp *input_time,
                        AudioBufferList *output, const AudioTimeStamp *output_time,
                        void *context) {
    (void)device; (void)now; (void)input_time; (void)output; (void)output_time;
    Capture *capture = context;
    if (atomic_load_explicit(&capture->changed, memory_order_acquire) || !input ||
        input->mNumberBuffers == 0) return noErr;
    const AudioBuffer *left = &input->mBuffers[0];
    if (!left->mData || left->mNumberChannels == 0) return noErr;
    const AudioBuffer *right = left;
    uint32_t right_offset = 0;
    if (left->mNumberChannels >= 2) {
        right_offset = 1;
    } else if (input->mNumberBuffers >= 2) {
        right = &input->mBuffers[1];
    }
    if (!right->mData || right->mNumberChannels == 0) return noErr;
    uint32_t left_frames = left->mDataByteSize / sizeof(float) / left->mNumberChannels;
    uint32_t right_frames = right->mDataByteSize / sizeof(float) / right->mNumberChannels;
    uint32_t frames = left_frames < right_frames ? left_frames : right_frames;
    capture->callback(capture->context, left->mData,
                      (const float *)right->mData + right_offset, frames,
                      left->mNumberChannels, right->mNumberChannels);
    return noErr;
}

// Only free context after callbacks/listeners are detached. On an OS teardown
// failure, retain the small allocation rather than risk a callback use-after-free.
int32_t symphos_capture_destroy(void *opaque) {
    Capture *capture = opaque;
    if (!capture) return noErr;
    OSStatus status;
    if (capture->started) {
        status = AudioDeviceStop(capture->device, capture->io);
        if (status != noErr) return status;
        capture->started = false;
    }
    if (capture->io) {
        status = AudioDeviceDestroyIOProcID(capture->device, capture->io);
        if (status != noErr) return status;
        capture->io = NULL;
    }
    if (capture->tap_listener) {
        AudioObjectPropertyAddress property = address(kAudioTapPropertyFormat);
        status = AudioObjectRemovePropertyListener(capture->tap, &property, changed, capture);
        if (status != noErr) return status;
        capture->tap_listener = false;
    }
    if (capture->output_listener) {
        AudioObjectPropertyAddress property = address(kAudioHardwarePropertyDefaultOutputDevice);
        status = AudioObjectRemovePropertyListener(kAudioObjectSystemObject, &property, changed, capture);
        if (status != noErr) return status;
        capture->output_listener = false;
    }
    if (capture->device) {
        status = AudioHardwareDestroyAggregateDevice(capture->device);
        if (status != noErr) return status;
        capture->device = 0;
    }
    if (capture->tap) {
        status = AudioHardwareDestroyProcessTap(capture->tap);
        if (status != noErr) return status;
        capture->tap = 0;
    }
    free(capture);
    return noErr;
}

// Always returns an owned handle when allocated, even on partial failure, so
// Rust can use the same teardown path. No capture begins until start is called.
int32_t symphos_capture_create(SampleCallback callback, void *context, void **result,
                               uint32_t *rate, char *error, uint32_t error_size) {
    @autoreleasepool {
        *result = NULL;
        if (@available(macOS 14.2, *)) {
            Capture *capture = calloc(1, sizeof(Capture));
            if (!capture) { snprintf(error, error_size, "Could not allocate capture state"); return -1; }
            *result = capture;
            atomic_init(&capture->changed, false);
            capture->callback = callback;
            capture->context = context;
            const char *step = "create system audio tap";
            CATapDescription *description = [[CATapDescription alloc]
                initStereoGlobalTapButExcludeProcesses:@[]];
            description.name = @"Symphos system audio";
            description.privateTap = YES;
            description.muteBehavior = CATapUnmuted;
            OSStatus status = AudioHardwareCreateProcessTap(description, &capture->tap);
            if (status != noErr) goto failed;

            step = "read tap format";
            AudioObjectPropertyAddress property = address(kAudioTapPropertyFormat);
            UInt32 size = sizeof(capture->format);
            status = AudioObjectGetPropertyData(capture->tap, &property, 0, NULL, &size, &capture->format);
            if (status != noErr) goto failed;
            AudioStreamBasicDescription format = capture->format;
            if (format.mFormatID != kAudioFormatLinearPCM ||
                !(format.mFormatFlags & kAudioFormatFlagIsFloat) ||
                !(format.mFormatFlags & kAudioFormatFlagIsPacked) ||
                (format.mFormatFlags & kAudioFormatFlagIsBigEndian) ||
                format.mBitsPerChannel != 32 || format.mChannelsPerFrame != 2 ||
                format.mSampleRate < 8000 || format.mSampleRate > 384000) {
                snprintf(error, error_size, "Unsupported system audio format (expected stereo Float32, 8–384 kHz)");
                return -1;
            }
            *rate = (uint32_t)format.mSampleRate;
            step = "create private capture device";
            status = AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)@{
                @kAudioAggregateDeviceNameKey: @"Symphos capture",
                @kAudioAggregateDeviceUIDKey: NSUUID.UUID.UUIDString,
                @kAudioAggregateDeviceIsPrivateKey: @YES,
                @kAudioAggregateDeviceTapAutoStartKey: @YES,
                @kAudioAggregateDeviceTapListKey: @[@{
                    @kAudioSubTapUIDKey: description.UUID.UUIDString,
                    @kAudioSubTapDriftCompensationKey: @YES
                }]
            }, &capture->device);
            if (status != noErr) goto failed;

            // The IOProc receives the aggregate's input format. Check it rather
            // than assuming its clock/packing is identical to the tap format.
            step = "read capture device format";
            property = (AudioObjectPropertyAddress){kAudioDevicePropertyStreamFormat,
                kAudioObjectPropertyScopeInput, kAudioObjectPropertyElementMain};
            size = sizeof(format);
            status = AudioObjectGetPropertyData(capture->device, &property, 0, NULL, &size, &format);
            if (status != noErr) goto failed;
            if (format.mFormatID != kAudioFormatLinearPCM ||
                !(format.mFormatFlags & kAudioFormatFlagIsFloat) ||
                !(format.mFormatFlags & kAudioFormatFlagIsPacked) ||
                (format.mFormatFlags & kAudioFormatFlagIsBigEndian) ||
                format.mBitsPerChannel != 32 || format.mChannelsPerFrame != 2 ||
                format.mSampleRate < 8000 || format.mSampleRate > 384000) {
                snprintf(error, error_size, "Unsupported capture device format");
                return -1;
            }
            *rate = (uint32_t)format.mSampleRate;
            step = "watch tap format";
            property = address(kAudioTapPropertyFormat);
            status = AudioObjectAddPropertyListener(capture->tap, &property, changed, capture);
            if (status != noErr) goto failed;
            capture->tap_listener = true;
            step = "watch output device";
            property = address(kAudioHardwarePropertyDefaultOutputDevice);
            status = AudioObjectAddPropertyListener(kAudioObjectSystemObject, &property, changed, capture);
            if (status != noErr) goto failed;
            capture->output_listener = true;
            step = "register capture callback";
            status = AudioDeviceCreateIOProcID(capture->device, io_proc, capture, &capture->io);
            if (status != noErr) goto failed;
            return noErr;
        failed:
            snprintf(error, error_size, "Could not %s (Core Audio status %d)", step, (int)status);
            return status;
        } else {
            snprintf(error, error_size, "System audio capture requires macOS 14.2 or newer");
            return -1;
        }
    }
}

int32_t symphos_capture_start(void *opaque) {
    Capture *capture = opaque;
    OSStatus status = AudioDeviceStart(capture->device, capture->io);
    capture->started = status == noErr;
    return status;
}

bool symphos_capture_changed(void *opaque) {
    return atomic_load_explicit(&((Capture *)opaque)->changed, memory_order_acquire);
}
