// host_gen/mod.rs
// AST를 분석해서 Metal 실행에 필요한 host-side 파일들을 자동 생성한다.
//
// 생성 파일:
//   common.h   — VertexAttributes enum, BufferIndex enum, uniform struct
//   main.mm    — Metal 보일러플레이트 (window, pipeline, draw loop)

use salvation_core::compiler::ast::types::{Item, Param, Program, ShaderStage, Type};

// ── 분석 결과 ────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ShaderInfo {
    pub vert_fn:   Option<String>,       // @vertex 함수명
    pub frag_fn:   Option<String>,       // @fragment 함수명
    pub uniforms:  Vec<UniformInfo>,     // uniform struct 목록
    pub structs:   Vec<StructInfo>,      // 일반 struct 목록
    pub vert_params: Vec<Param>,         // vertex 함수 파라미터
}

#[derive(Debug, Clone)]
pub struct UniformInfo {
    pub name:   String,
    pub fields: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name:   String,
    pub fields: Vec<Param>,
}

// ── AST 분석 ─────────────────────────────────────────────────

pub fn analyze(program: &Program) -> ShaderInfo {
    let mut info = ShaderInfo::default();

    for item in program {
        match item {
            Item::FnDecl { stage, name, params, .. } => {
                match stage {
                    Some(ShaderStage::Vertex) => {
                        info.vert_fn = Some(name.clone());
                        info.vert_params = params.clone();
                    }
                    Some(ShaderStage::Fragment) => {
                        info.frag_fn = Some(name.clone());
                    }
                    _ => {}
                }
            }
            Item::StructDecl { name, fields } => {
                info.structs.push(StructInfo {
                    name: name.clone(),
                    fields: fields.clone(),
                });
            }
            _ => {}
        }
    }

    info
}

// ── common.h 생성 ─────────────────────────────────────────────

pub fn gen_common_h(info: &ShaderInfo) -> String {
    let mut out = String::new();

    out.push_str("#ifndef COMMON_H\n");
    out.push_str("#define COMMON_H\n\n");
    out.push_str("#include <simd/simd.h>\n\n");

    // VertexAttributes enum — vertex 파라미터에서 추출
    out.push_str("enum VertexAttributes {\n");
    for (i, p) in info.vert_params.iter().enumerate() {
        let variant = to_upper_camel("VertexAttribute", &p.name);
        out.push_str(&format!("    {} = {},\n", variant, i));
    }
    out.push_str("};\n\n");

    // BufferIndex enum
    out.push_str("enum BufferIndex {\n");
    out.push_str("    MeshVertexBuffer   = 0,\n");
    out.push_str("    FrameUniformBuffer = 1,\n");
    out.push_str("};\n\n");

    // FrameUniforms struct — 첫 번째 struct를 uniform으로 사용
    // (나중에 @uniform 어트리뷰트로 명시하는 방식으로 확장 예정)
    if let Some(s) = info.structs.first() {
        out.push_str(&format!("struct {} {{\n", s.name));
        for f in &s.fields {
            out.push_str(&format!("    {} {};\n", emit_type_cpp(&f.ty), f.name));
        }
        out.push_str("};\n\n");
    } else {
        // struct 선언이 없으면 기본 FrameUniforms 생성
        out.push_str("struct FrameUniforms {\n");
        out.push_str("    simd::float4x4 projectionViewModel;\n");
        out.push_str("};\n\n");
    }

    out.push_str("#endif\n");
    out
}

// ── main.mm 생성 ──────────────────────────────────────────────

pub fn gen_main_mm(info: &ShaderInfo, metallib_name: &str) -> String {
    let vert = info.vert_fn.as_deref().unwrap_or("vert");
    let frag  = info.frag_fn.as_deref().unwrap_or("frag");

    // vertex 파라미터에서 MTLVertexDescriptor 설정 코드 생성
    let vert_desc = gen_vertex_descriptor(&info.vert_params);

    // uniform struct 이름 (첫 번째 struct 또는 기본값)
    let uniform_struct = info.structs.first()
        .map(|s| s.name.as_str())
        .unwrap_or("FrameUniforms");

    format!(r#"#import <Cocoa/Cocoa.h>
#import <Metal/Metal.h>
#import <MetalKit/MetalKit.h>
#import <simd/simd.h>
#import "common.h"

@interface SalvationView : MTKView
@end

int main() {{
    @autoreleasepool {{
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
    }}
    return 0;
}}

// ── CPU-side vertex ──────────────────────────────────────────
struct Vertex {{
    simd::float4 position;
    simd::float4 color;
}};

constexpr int kUniformBufferCount = 3;

@implementation SalvationView {{
    id<MTLLibrary>             _library;
    id<MTLCommandQueue>        _commandQueue;
    id<MTLRenderPipelineState> _pipelineState;
    id<MTLDepthStencilState>   _depthState;
    dispatch_semaphore_t       _semaphore;
    id<MTLBuffer>              _uniformBuffers[kUniformBufferCount];
    id<MTLBuffer>              _vertexBuffer;
    int   _uniformBufferIndex;
    long  _frame;
}}

- (id)initWithFrame:(CGRect)frame {{
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    self = [super initWithFrame:frame device:device];
    if (self) {{ [self _setup]; }}
    return self;
}}

- (void)_setup {{
    self.colorPixelFormat        = MTLPixelFormatBGRA8Unorm;
    self.depthStencilPixelFormat = MTLPixelFormatDepth32Float_Stencil8;

    NSError* error = nil;
    NSString* libPath = [[[NSBundle mainBundle] bundlePath]
                          stringByAppendingPathComponent:@"{metallib_name}"];
    _library = [self.device newLibraryWithFile:libPath error:&error];
    if (!_library) {{
        _library = [self.device newLibraryWithFile:@"{metallib_name}" error:&error];
    }}
    if (!_library) {{
        NSLog(@"Failed to load library: %@", error);
        exit(1);
    }}

    id<MTLFunction> vertFunc = [_library newFunctionWithName:@"{vert}"];
    id<MTLFunction> fragFunc = [_library newFunctionWithName:@"{frag}"];

    MTLDepthStencilDescriptor* depthDesc  = [MTLDepthStencilDescriptor new];
    depthDesc.depthCompareFunction        = MTLCompareFunctionLess;
    depthDesc.depthWriteEnabled           = YES;
    _depthState = [self.device newDepthStencilStateWithDescriptor:depthDesc];

    MTLVertexDescriptor* vertDesc = [MTLVertexDescriptor new];
{vert_desc}
    MTLRenderPipelineDescriptor* pipelineDesc = [MTLRenderPipelineDescriptor new];
    pipelineDesc.sampleCount             = self.sampleCount;
    pipelineDesc.vertexFunction          = vertFunc;
    pipelineDesc.fragmentFunction        = fragFunc;
    pipelineDesc.vertexDescriptor        = vertDesc;
    pipelineDesc.colorAttachments[0].pixelFormat  = self.colorPixelFormat;
    pipelineDesc.depthAttachmentPixelFormat        = self.depthStencilPixelFormat;
    pipelineDesc.stencilAttachmentPixelFormat      = self.depthStencilPixelFormat;
    _pipelineState = [self.device newRenderPipelineStateWithDescriptor:pipelineDesc error:&error];
    if (!_pipelineState) {{
        NSLog(@"Failed to create pipeline: %@", error);
        exit(1);
    }}

    Vertex verts[] = {{
        {{ {{-0.5f, -0.5f, 0.0f, 1.0f}}, {{1.0f, 0.0f, 0.0f, 1.0f}} }},
        {{ {{ 0.0f,  0.5f, 0.0f, 1.0f}}, {{0.0f, 1.0f, 0.0f, 1.0f}} }},
        {{ {{ 0.5f, -0.5f, 0.0f, 1.0f}}, {{0.0f, 0.0f, 1.0f, 1.0f}} }},
    }};
    _vertexBuffer = [self.device newBufferWithBytes:verts
                                             length:sizeof(verts)
                                            options:MTLResourceStorageModeShared];

    for (int i = 0; i < kUniformBufferCount; i++) {{
        _uniformBuffers[i] = [self.device
                               newBufferWithLength:sizeof({uniform_struct})
                               options:MTLResourceCPUCacheModeWriteCombined];
    }}

    _semaphore          = dispatch_semaphore_create(kUniformBufferCount);
    _uniformBufferIndex = 0;
    _frame              = 0;
    _commandQueue       = [self.device newCommandQueue];
}}

- (void)drawRect:(CGRect)rect {{
    dispatch_semaphore_wait(_semaphore, DISPATCH_TIME_FOREVER);

    _frame++;
    float rad = _frame * 0.01f;
    float s = std::sin(rad), c = std::cos(rad);
    simd::float4x4 rot(
        simd::float4{{ c, -s, 0, 0}},
        simd::float4{{ s,  c, 0, 0}},
        simd::float4{{ 0,  0, 1, 0}},
        simd::float4{{ 0,  0, 0, 1}}
    );

    _uniformBufferIndex = (_uniformBufferIndex + 1) % kUniformBufferCount;
    {uniform_struct}* uniforms = ({uniform_struct}*)[_uniformBuffers[_uniformBufferIndex] contents];
    uniforms->projectionViewModel = rot;

    id<MTLCommandBuffer>       cmd     = [_commandQueue commandBuffer];
    id<MTLRenderCommandEncoder> encoder =
        [cmd renderCommandEncoderWithDescriptor:self.currentRenderPassDescriptor];

    [encoder setViewport:{{0, 0,
                          (double)self.drawableSize.width,
                          (double)self.drawableSize.height,
                          0, 1}}];
    [encoder setDepthStencilState:_depthState];
    [encoder setRenderPipelineState:_pipelineState];
    [encoder setVertexBuffer:_uniformBuffers[_uniformBufferIndex]
                      offset:0 atIndex:FrameUniformBuffer];
    [encoder setVertexBuffer:_vertexBuffer offset:0 atIndex:MeshVertexBuffer];
    [encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
    [encoder endEncoding];

    __block dispatch_semaphore_t sem = _semaphore;
    [cmd addCompletedHandler:^(id<MTLCommandBuffer> _) {{
        dispatch_semaphore_signal(sem);
    }}];
    [cmd presentDrawable:self.currentDrawable];
    [cmd commit];

    [super drawRect:rect];
}}

@end
"#,
        metallib_name = metallib_name,
        vert          = vert,
        frag          = frag,
        vert_desc     = vert_desc,
        uniform_struct = uniform_struct,
    )
}

// ── 헬퍼: vertex descriptor 코드 생성 ────────────────────────

fn gen_vertex_descriptor(params: &[Param]) -> String {
    if params.is_empty() {
        return concat!(
            "    vertDesc.attributes[VertexAttributePosition].format      = MTLVertexFormatFloat4;\n",
            "    vertDesc.attributes[VertexAttributePosition].offset      = 0;\n",
            "    vertDesc.attributes[VertexAttributePosition].bufferIndex = MeshVertexBuffer;\n",
            "    vertDesc.attributes[VertexAttributeColor].format         = MTLVertexFormatFloat4;\n",
            "    vertDesc.attributes[VertexAttributeColor].offset         = 16;\n",
            "    vertDesc.attributes[VertexAttributeColor].bufferIndex    = MeshVertexBuffer;\n",
            "    vertDesc.layouts[MeshVertexBuffer].stride                = 32;\n",
            "    vertDesc.layouts[MeshVertexBuffer].stepRate              = 1;\n",
            "    vertDesc.layouts[MeshVertexBuffer].stepFunction          = MTLVertexStepFunctionPerVertex;\n",
        ).to_string();
    }

    let mut out = String::new();
    let mut offset = 0usize;
    for (i, p) in params.iter().enumerate() {
        let attr = to_upper_camel("VertexAttribute", &p.name);
        let (fmt, size) = metal_vertex_format(&p.ty);
        out.push_str(&format!(
            "    vertDesc.attributes[{attr}].format      = {fmt};\n",
            attr = attr, fmt = fmt,
        ));
        out.push_str(&format!(
            "    vertDesc.attributes[{attr}].offset      = {offset};\n",
            attr = attr, offset = offset,
        ));
        out.push_str(&format!(
            "    vertDesc.attributes[{attr}].bufferIndex = MeshVertexBuffer;\n",
            attr = attr,
        ));
        offset += size;
        let _ = i;
    }
    out.push_str(&format!(
        "    vertDesc.layouts[MeshVertexBuffer].stride       = {offset};\n",
        offset = offset,
    ));
    out.push_str(
        "    vertDesc.layouts[MeshVertexBuffer].stepRate     = 1;\n"
    );
    out.push_str(
        "    vertDesc.layouts[MeshVertexBuffer].stepFunction = MTLVertexStepFunctionPerVertex;\n"
    );
    out
}

// ── 헬퍼: 타입 변환 ──────────────────────────────────────────

fn emit_type_cpp(ty: &Type) -> &'static str {
    match ty {
        Type::Bool    => "bool",
        Type::Int     => "int",
        Type::Uint    => "unsigned int",
        Type::Float   => "float",
        Type::Float2  => "simd::float2",
        Type::Float3  => "simd::float3",
        Type::Float4  => "simd::float4",
        Type::Mat2x2  => "simd::float2x2",
        Type::Mat3x3  => "simd::float3x3",
        Type::Mat4x4  => "simd::float4x4",
        Type::Mat4x3  => "simd::float4x3",
        Type::Mat3x4  => "simd::float3x4",
        _             => "float",
    }
}

fn metal_vertex_format(ty: &Type) -> (&'static str, usize) {
    match ty {
        Type::Float   => ("MTLVertexFormatFloat",  4),
        Type::Float2  => ("MTLVertexFormatFloat2", 8),
        Type::Float3  => ("MTLVertexFormatFloat3", 12),
        Type::Float4  => ("MTLVertexFormatFloat4", 16),
        Type::Int     => ("MTLVertexFormatInt",    4),
        Type::Uint    => ("MTLVertexFormatUInt",   4),
        _             => ("MTLVertexFormatFloat4", 16),
    }
}

fn to_upper_camel(prefix: &str, name: &str) -> String {
    let cap: String = name.chars().enumerate()
        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
        .collect();
    format!("{}{}", prefix, cap)
}