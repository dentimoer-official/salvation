#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#import <MetalKit/MetalKit.h>
#import <simd/simd.h>
#import "common.h"

@interface SalvationView : MTKView
@end

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        [NSApp activateIgnoringOtherApps:YES];

        NSMenu* bar      = [NSMenu new];
        NSMenuItem* item = [NSMenuItem new];
        NSMenu* menu     = [NSMenu new];
        NSMenuItem* quit = [[NSMenuItem alloc]
                               initWithTitle:@"Quit"
                               action:@selector(terminate:)
                               keyEquivalent:@"q"];
        [bar addItem:item];
        [item setSubmenu:menu];
        [menu addItem:quit];
        NSApp.mainMenu = bar;

        NSRect frame = NSMakeRect(0, 0, 512, 512);
        NSWindow* window = [[NSWindow alloc]
                               initWithContentRect:frame
                               styleMask:NSTitledWindowMask
                               backing:NSBackingStoreBuffered
                               defer:NO];
        [window cascadeTopLeftFromPoint:NSMakePoint(20, 20)];
        window.title = [[NSProcessInfo processInfo] processName];
        [window makeKeyAndOrderFront:nil];

        SalvationView* view = [[SalvationView alloc] initWithFrame:frame];
        window.contentView = view;

        [NSApp run];
    }
    return 0;
}

// ── CPU-side vertex ──────────────────────────────────────────
struct Vertex {
    simd::float4 position;
    simd::float4 color;
};

constexpr int kUniformBufferCount = 3;

@implementation SalvationView {
    id<MTLLibrary>             _library;
    id<MTLCommandQueue>        _commandQueue;
    id<MTLRenderPipelineState> _pipelineState;
    id<MTLDepthStencilState>   _depthState;
    dispatch_semaphore_t       _semaphore;
    id<MTLBuffer>              _uniformBuffers[kUniformBufferCount];
    id<MTLBuffer>              _vertexBuffer;
    int   _uniformBufferIndex;
    long  _frame;
}

- (id)initWithFrame:(CGRect)frame {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    self = [super initWithFrame:frame device:device];
    if (self) { [self _setup]; }
    return self;
}

- (void)_setup {
    self.colorPixelFormat        = MTLPixelFormatBGRA8Unorm;
    self.depthStencilPixelFormat = MTLPixelFormatDepth32Float_Stencil8;

    NSError* error = nil;
    NSString* libPath = [[[NSBundle mainBundle] bundlePath]
                          stringByAppendingPathComponent:@"shaders.metallib"];
    _library = [self.device newLibraryWithFile:libPath error:&error];
    if (!_library) {
        _library = [self.device newLibraryWithFile:@"shaders.metallib" error:&error];
    }
    if (!_library) {
        NSLog(@"Failed to load library: %@", error);
        exit(1);
    }

    id<MTLFunction> vertFunc = [_library newFunctionWithName:@"vert"];
    id<MTLFunction> fragFunc = [_library newFunctionWithName:@"frag"];

    MTLDepthStencilDescriptor* depthDesc  = [MTLDepthStencilDescriptor new];
    depthDesc.depthCompareFunction        = MTLCompareFunctionLess;
    depthDesc.depthWriteEnabled           = YES;
    _depthState = [self.device newDepthStencilStateWithDescriptor:depthDesc];

    MTLVertexDescriptor* vertDesc = [MTLVertexDescriptor new];
    vertDesc.attributes[VertexAttributePosition].format      = MTLVertexFormatFloat4;
    vertDesc.attributes[VertexAttributePosition].offset      = 0;
    vertDesc.attributes[VertexAttributePosition].bufferIndex = MeshVertexBuffer;
    vertDesc.attributes[VertexAttributeColor].format      = MTLVertexFormatFloat4;
    vertDesc.attributes[VertexAttributeColor].offset      = 16;
    vertDesc.attributes[VertexAttributeColor].bufferIndex = MeshVertexBuffer;
    vertDesc.layouts[MeshVertexBuffer].stride       = 32;
    vertDesc.layouts[MeshVertexBuffer].stepRate     = 1;
    vertDesc.layouts[MeshVertexBuffer].stepFunction = MTLVertexStepFunctionPerVertex;

    MTLRenderPipelineDescriptor* pipelineDesc = [MTLRenderPipelineDescriptor new];
    pipelineDesc.sampleCount             = self.sampleCount;
    pipelineDesc.vertexFunction          = vertFunc;
    pipelineDesc.fragmentFunction        = fragFunc;
    pipelineDesc.vertexDescriptor        = vertDesc;
    pipelineDesc.colorAttachments[0].pixelFormat  = self.colorPixelFormat;
    pipelineDesc.depthAttachmentPixelFormat        = self.depthStencilPixelFormat;
    pipelineDesc.stencilAttachmentPixelFormat      = self.depthStencilPixelFormat;
    _pipelineState = [self.device newRenderPipelineStateWithDescriptor:pipelineDesc error:&error];
    if (!_pipelineState) {
        NSLog(@"Failed to create pipeline: %@", error);
        exit(1);
    }

    Vertex verts[] = {
        { {-0.5f, -0.5f, 0.0f, 1.0f}, {1.0f, 0.0f, 0.0f, 1.0f} },
        { { 0.0f,  0.5f, 0.0f, 1.0f}, {0.0f, 1.0f, 0.0f, 1.0f} },
        { { 0.5f, -0.5f, 0.0f, 1.0f}, {0.0f, 0.0f, 1.0f, 1.0f} },
    };
    _vertexBuffer = [self.device newBufferWithBytes:verts
                                             length:sizeof(verts)
                                            options:MTLResourceStorageModeShared];

    for (int i = 0; i < kUniformBufferCount; i++) {
        _uniformBuffers[i] = [self.device
                               newBufferWithLength:sizeof(FrameUniforms)
                               options:MTLResourceCPUCacheModeWriteCombined];
    }

    _semaphore          = dispatch_semaphore_create(kUniformBufferCount);
    _uniformBufferIndex = 0;
    _frame              = 0;
    _commandQueue       = [self.device newCommandQueue];
}

- (void)drawRect:(CGRect)rect {
    dispatch_semaphore_wait(_semaphore, DISPATCH_TIME_FOREVER);

    _frame++;
    float rad = _frame * 0.01f;
    float s = std::sin(rad), c = std::cos(rad);
    simd::float4x4 rot(
        simd::float4{ c, -s, 0, 0},
        simd::float4{ s,  c, 0, 0},
        simd::float4{ 0,  0, 1, 0},
        simd::float4{ 0,  0, 0, 1}
    );

    _uniformBufferIndex = (_uniformBufferIndex + 1) % kUniformBufferCount;
    FrameUniforms* uniforms = (FrameUniforms*)[_uniformBuffers[_uniformBufferIndex] contents];
    uniforms->projectionViewModel = rot;

    id<MTLCommandBuffer>       cmd     = [_commandQueue commandBuffer];
    id<MTLRenderCommandEncoder> encoder =
        [cmd renderCommandEncoderWithDescriptor:self.currentRenderPassDescriptor];

    [encoder setViewport:{0, 0,
                          (double)self.drawableSize.width,
                          (double)self.drawableSize.height,
                          0, 1}];
    [encoder setDepthStencilState:_depthState];
    [encoder setRenderPipelineState:_pipelineState];
    [encoder setVertexBuffer:_uniformBuffers[_uniformBufferIndex]
                      offset:0 atIndex:FrameUniformBuffer];
    [encoder setVertexBuffer:_vertexBuffer offset:0 atIndex:MeshVertexBuffer];
    [encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
    [encoder endEncoding];

    __block dispatch_semaphore_t sem = _semaphore;
    [cmd addCompletedHandler:^(id<MTLCommandBuffer> _) {
        dispatch_semaphore_signal(sem);
    }];
    [cmd presentDrawable:self.currentDrawable];
    [cmd commit];

    [super drawRect:rect];
}

@end
