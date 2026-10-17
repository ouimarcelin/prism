// Prism's side of Syphon: a server publishing Metal textures by copying
// them into the shared IOSurface, and a C interface for Rust.
//
// Only the copy path of Syphon's own Metal server is used: Prism hands over
// a BGRA8 texture of the right size, so no shader is needed.

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <IOSurface/IOSurface.h>
#import "SyphonServerBase.h"
#import "SyphonSubclassing.h"

@interface PrismSyphonServer : SyphonServerBase
@end

@implementation PrismSyphonServer
{
    id<MTLDevice> _device;
    id<MTLTexture> _surfaceTexture;
}

- (instancetype)initWithName:(NSString *)name device:(id<MTLDevice>)device
{
    self = [super initWithName:name options:nil];
    if (self) {
        _device = device;
    }
    return self;
}

- (id<MTLTexture>)surfaceOfWidth:(NSUInteger)width height:(NSUInteger)height
{
    if (_surfaceTexture && (_surfaceTexture.width != width || _surfaceTexture.height != height)) {
        _surfaceTexture = nil;
    }
    if (!_surfaceTexture) {
        MTLTextureDescriptor *descriptor =
            [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                                               width:width
                                                              height:height
                                                           mipmapped:NO];
        descriptor.usage = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
        IOSurfaceRef surface = [self newSurfaceForWidth:width height:height options:nil];
        if (surface) {
            _surfaceTexture = [_device newTextureWithDescriptor:descriptor iosurface:surface plane:0];
            CFRelease(surface);
        }
    }
    return _surfaceTexture;
}

- (void)publishTexture:(id<MTLTexture>)texture onQueue:(id<MTLCommandQueue>)queue
{
    id<MTLTexture> destination = [self surfaceOfWidth:texture.width height:texture.height];
    if (!destination) {
        return;
    }
    id<MTLCommandBuffer> commands = [queue commandBuffer];
    id<MTLBlitCommandEncoder> blit = [commands blitCommandEncoder];
    [blit copyFromTexture:texture
              sourceSlice:0
              sourceLevel:0
             sourceOrigin:MTLOriginMake(0, 0, 0)
               sourceSize:MTLSizeMake(texture.width, texture.height, 1)
                toTexture:destination
         destinationSlice:0
         destinationLevel:0
        destinationOrigin:MTLOriginMake(0, 0, 0)];
    [blit endEncoding];
    __weak PrismSyphonServer *weakSelf = self;
    [commands addCompletedHandler:^(id<MTLCommandBuffer> _Nonnull buffer) {
        [weakSelf publish];
    }];
    [commands commit];
}

- (void)stop
{
    _surfaceTexture = nil;
    [super stop];
}

@end

void *prism_syphon_server_new(const char *name, void *device)
{
    @autoreleasepool {
        NSString *title = [NSString stringWithUTF8String:name];
        PrismSyphonServer *server =
            [[PrismSyphonServer alloc] initWithName:title device:(__bridge id<MTLDevice>)device];
        return server ? (__bridge_retained void *)server : NULL;
    }
}

void prism_syphon_server_publish(void *server, void *texture, void *queue)
{
    @autoreleasepool {
        PrismSyphonServer *object = (__bridge PrismSyphonServer *)server;
        [object publishTexture:(__bridge id<MTLTexture>)texture onQueue:(__bridge id<MTLCommandQueue>)queue];
    }
}

int prism_syphon_server_has_clients(void *server)
{
    PrismSyphonServer *object = (__bridge PrismSyphonServer *)server;
    return object.hasClients ? 1 : 0;
}

void prism_syphon_server_free(void *server)
{
    @autoreleasepool {
        PrismSyphonServer *object = (__bridge_transfer PrismSyphonServer *)server;
        [object stop];
        object = nil;
    }
}
