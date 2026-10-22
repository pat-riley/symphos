#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <string.h>

@interface SymphosRecorder : NSObject
@property AVAssetWriter *writer;
@property AVAssetWriterInput *video;
@property AVAssetWriterInput *audio;
@property AVAssetWriterInputPixelBufferAdaptor *pixels;
@property int width;
@property int height;
@property int rate;
@end
@implementation SymphosRecorder
@end
static void report(char *error, size_t size, NSString *message) {
    if (error && size) snprintf(error, size, "%s", (message ?: @"Recording failed").UTF8String);
}
char *symphos_record_choose(void) {
    NSSavePanel *panel = [NSSavePanel savePanel];
    panel.title = @"Record Visualizer";
    panel.allowedContentTypes = @[UTTypeQuickTimeMovie];
    panel.nameFieldStringValue = @"Symphos performance.mov";
    panel.canCreateDirectories = YES;
    if ([panel runModal] != NSModalResponseOK) return NULL;
    return strdup(panel.URL.path.UTF8String);
}
void *symphos_record_open(const char *path, int width, int height, int fps, int rate, char *error, size_t size) {
    @autoreleasepool {
        SymphosRecorder *r = [SymphosRecorder new];
        NSError *failure = nil;
        r.writer = [[AVAssetWriter alloc] initWithURL:[NSURL fileURLWithPath:[NSString stringWithUTF8String:path]] fileType:AVFileTypeQuickTimeMovie error:&failure];
        if (!r.writer) { report(error,size,failure.localizedDescription); return NULL; }
        r.width = width; r.height = height; r.rate = rate;
        NSDictionary *video = @{AVVideoCodecKey:AVVideoCodecTypeH264, AVVideoWidthKey:@(width), AVVideoHeightKey:@(height),
            AVVideoCompressionPropertiesKey:@{AVVideoAverageBitRateKey:@(MAX(8000000, (long long)width*height*fps/5)), AVVideoExpectedSourceFrameRateKey:@(fps), AVVideoMaxKeyFrameIntervalKey:@(fps*2)}};
        NSDictionary *audio = @{AVFormatIDKey:@(kAudioFormatMPEG4AAC),AVSampleRateKey:@(rate),AVNumberOfChannelsKey:@2,AVEncoderBitRateKey:@256000};
        if (![r.writer canApplyOutputSettings:video forMediaType:AVMediaTypeVideo] ||
            ![r.writer canApplyOutputSettings:audio forMediaType:AVMediaTypeAudio]) {
            report(error,size,@"Encoder cannot support this video size, frame rate, or audio format");
            return NULL;
        }
        r.video = [AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeVideo outputSettings:video];
        r.video.expectsMediaDataInRealTime = YES;
        r.pixels = [AVAssetWriterInputPixelBufferAdaptor assetWriterInputPixelBufferAdaptorWithAssetWriterInput:r.video sourcePixelBufferAttributes:@{(NSString *)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32BGRA), (NSString *)kCVPixelBufferWidthKey:@(width), (NSString *)kCVPixelBufferHeightKey:@(height)}];
        r.audio = [AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeAudio outputSettings:audio];
        r.audio.expectsMediaDataInRealTime = YES;
        if (![r.writer canAddInput:r.video] || ![r.writer canAddInput:r.audio]) { report(error,size,@"Encoder cannot support this output configuration"); return NULL; }
        [r.writer addInput:r.video]; [r.writer addInput:r.audio];
        if (![r.writer startWriting]) { report(error,size,r.writer.error.localizedDescription); return NULL; }
        [r.writer startSessionAtSourceTime:kCMTimeZero];
        return (__bridge_retained void *)r;
    }
}
int symphos_record_video(void *handle, const uint8_t *bytes, double seconds, char *error, size_t size) {
    @autoreleasepool {
        SymphosRecorder *r = (__bridge SymphosRecorder *)handle;
        if (r.writer.status == AVAssetWriterStatusFailed) { report(error,size,r.writer.error.localizedDescription); return -1; }
        if (!r.video.readyForMoreMediaData) return 0;
        CVPixelBufferRef pixel = NULL;
        CVReturn result = CVPixelBufferPoolCreatePixelBuffer(NULL, r.pixels.pixelBufferPool, &pixel);
        if (result != kCVReturnSuccess) { report(error,size,@"Could not allocate encoder frame"); return -1; }
        CVPixelBufferLockBaseAddress(pixel, 0);
        uint8_t *dest = CVPixelBufferGetBaseAddress(pixel);
        size_t stride = CVPixelBufferGetBytesPerRow(pixel);
        for (int y=0; y<r.height; ++y) memcpy(dest+y*stride, bytes+(size_t)y*r.width*4, (size_t)r.width*4);
        CVPixelBufferUnlockBaseAddress(pixel,0);
        BOOL ok = [r.pixels appendPixelBuffer:pixel withPresentationTime:CMTimeMakeWithSeconds(seconds,600000)];
        CVPixelBufferRelease(pixel);
        if (!ok) { report(error,size,r.writer.error.localizedDescription); return -1; }
        return 1;
    }
}
int symphos_record_audio(void *handle, const float *samples, int frames, double seconds, char *error, size_t size) {
    @autoreleasepool {
        SymphosRecorder *r = (__bridge SymphosRecorder *)handle;
        if (r.writer.status == AVAssetWriterStatusFailed) { report(error,size,r.writer.error.localizedDescription); return -1; }
        if (!r.audio.readyForMoreMediaData) return 0;
        AudioStreamBasicDescription asbd = {0};
        asbd.mSampleRate=r.rate; asbd.mFormatID=kAudioFormatLinearPCM;
        asbd.mFormatFlags=kAudioFormatFlagIsFloat|kAudioFormatFlagIsPacked;
        asbd.mBytesPerPacket=8; asbd.mFramesPerPacket=1; asbd.mBytesPerFrame=8; asbd.mChannelsPerFrame=2; asbd.mBitsPerChannel=32;
        CMAudioFormatDescriptionRef format = NULL;
        CMBlockBufferRef block = NULL;
        CMSampleBufferRef sample = NULL;
        OSStatus status = CMAudioFormatDescriptionCreate(NULL,&asbd,0,NULL,0,NULL,NULL,&format);
        size_t length=(size_t)frames*8;
        if (!status) status=CMBlockBufferCreateWithMemoryBlock(NULL,NULL,length,NULL,NULL,0,length,0,&block);
        if (!status) status=CMBlockBufferReplaceDataBytes(samples,block,0,length);
        CMSampleTimingInfo timing = {CMTimeMake(1,r.rate),CMTimeMakeWithSeconds(seconds,600000),kCMTimeInvalid};
        if (!status) status=CMSampleBufferCreateReady(NULL,block,format,frames,1,&timing,0,NULL,&sample);
        BOOL ok = !status && [r.audio appendSampleBuffer:sample];
        if (sample) CFRelease(sample); if (block) CFRelease(block); if (format) CFRelease(format);
        if (!ok) { report(error,size,r.writer.error.localizedDescription ?: @"Could not encode audio buffer"); return -1; }
        return 1;
    }
}
int symphos_record_finish(void *handle, double seconds, char *error, size_t size) {
    @autoreleasepool {
        SymphosRecorder *r = (__bridge_transfer SymphosRecorder *)handle;
        [r.writer endSessionAtSourceTime:CMTimeMakeWithSeconds(seconds,600000)];
        [r.video markAsFinished]; [r.audio markAsFinished];
        dispatch_semaphore_t done=dispatch_semaphore_create(0);
        [r.writer finishWritingWithCompletionHandler:^{ dispatch_semaphore_signal(done); }];
        if (dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,30*NSEC_PER_SEC)) != 0) {
            [r.writer cancelWriting];
            report(error,size,@"Video encoder timed out while finalizing the movie");
            return 0;
        }
        if (r.writer.status != AVAssetWriterStatusCompleted) { report(error,size,r.writer.error.localizedDescription); return 0; }
        return 1;
    }
}
void symphos_record_cancel(void *handle) {
    @autoreleasepool {
        SymphosRecorder *r = (__bridge_transfer SymphosRecorder *)handle;
        [r.writer cancelWriting];
    }
}
