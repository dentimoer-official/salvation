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

// ── shader_types.h 생성 (CPU/GPU 공유 타입 헤더) ──────────────
// Metal 셰이더(.metal)와 C++ 호스트(main.mm) 양쪽에서 include 가능한 헤더.
// sv_* alias 제거 — 각 플랫폼 고유 타입을 struct 필드에 직접 분기해 사용.
//   Metal  : float4x4, float4, float3, float2 (metal_stdlib)
//   C++ host: simd::float4x4, simd::float4, ... (simd/simd.h)
// 두 타입은 메모리 레이아웃이 동일하므로 별도 alias 없이 바로 공유 가능.

pub fn gen_shader_types_h(info: &ShaderInfo) -> String {
    let mut out = String::new();

    out.push_str("#ifndef SHADER_TYPES_H\n");
    out.push_str("#define SHADER_TYPES_H\n\n");
    out.push_str("#ifdef __METAL_VERSION__\n");
    out.push_str("#  include <metal_stdlib>\n");
    out.push_str("   using namespace metal;\n");
    out.push_str("#else\n");
    out.push_str("#  include <simd/simd.h>\n");
    out.push_str("#endif\n\n");

    // .slvt struct → CPU/GPU 플랫폼별 타입으로 직접 분기
    if info.structs.is_empty() {
        // 기본 uniform struct
        out.push_str("struct FrameUniforms {\n");
        out.push_str("#ifdef __METAL_VERSION__\n");
        out.push_str("    float4x4 projectionViewModel;\n");
        out.push_str("#else\n");
        out.push_str("    simd::float4x4 projectionViewModel;\n");
        out.push_str("#endif\n");
        out.push_str("};\n\n");
    } else {
        for s in &info.structs {
            out.push_str(&format!("struct {} {{\n", s.name));
            out.push_str("#ifdef __METAL_VERSION__\n");
            for f in &s.fields {
                out.push_str(&format!("    {} {};\n", emit_type_metal(&f.ty), f.name));
            }
            out.push_str("#else\n");
            for f in &s.fields {
                out.push_str(&format!("    {} {};\n", emit_type_cpp(&f.ty), f.name));
            }
            out.push_str("#endif\n");
            out.push_str("};\n\n");
        }
    }

    out.push_str("#endif /* SHADER_TYPES_H */\n");
    out
}

// ── common.h 생성 ─────────────────────────────────────────────

pub fn gen_common_h(info: &ShaderInfo) -> String {
    let mut out = String::new();

    out.push_str("#ifndef COMMON_H\n");
    out.push_str("#define COMMON_H\n\n");
    // [Fix 5] struct 정의를 shader_types.h 한 곳에서만 관리.
    // 이전에는 common.h와 shaders.metal 양쪽에 중복 선언되어 유지보수 지옥이었음.
    out.push_str("#include \"shader_types.h\"\n\n");

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

    out.push_str("#endif /* COMMON_H */\n");
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
#import <QuartzCore/QuartzCore.h>
#import <simd/simd.h>
#include <cmath>
#import "common.h"

// ── SalvationView 전방 전체 선언 ─────────────────────────────
// @class 전방 선언만으로는 alloc/init 호출이 불가하므로
// @interface 전체를 AppDelegate 앞에 배치.
// [Fix 3-2] MTKViewDelegate 채택: CADisplayLink 기반 고성능 렌더링 루프 사용.
@interface SalvationView : MTKView <MTKViewDelegate>
@end

// ── AppDelegate ───────────────────────────────────────────────
// [Fix 2-2] NSWindow를 strong property로 관리하여 수명 보장.
@interface AppDelegate : NSObject <NSApplicationDelegate>
@property (strong) NSWindow* window;
@end

@implementation AppDelegate
- (void)applicationDidFinishLaunching:(NSNotification*)notification {{
    NSRect frame = NSMakeRect(0, 0, 512, 512);
    self.window = [[NSWindow alloc]
                      initWithContentRect:frame
                      styleMask:NSWindowStyleMaskTitled
                      backing:NSBackingStoreBuffered
                      defer:NO];
    [self.window cascadeTopLeftFromPoint:NSMakePoint(20, 20)];
    self.window.title = [[NSProcessInfo processInfo] processName];
    SalvationView* view = [[SalvationView alloc] initWithFrame:frame];
    self.window.contentView = view;
    [self.window makeKeyAndOrderFront:nil];
}}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)sender {{
    return YES;
}}
@end

int main() {{
    @autoreleasepool {{
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

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

        AppDelegate* delegate = [AppDelegate new];
        NSApp.delegate = delegate;

        [NSApp activateIgnoringOtherApps:YES];
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

// ── 렌더링 수학 헬퍼 ─────────────────────────────────────────
// Z축 CCW(반시계) 회전 행렬 (column-major).
// simd::float4x4(col0, col1, ...) 에서 인자 순서 = 열(column) 순서.
//   col0 = {{  c, s, 0, 0 }}  ← 첫 번째 열
//   col1 = {{ -s, c, 0, 0 }}  ← 두 번째 열
// M * v 전개 (표준 CCW 검증):
//   result.x = col0.x·vx + col1.x·vy =  c·vx − s·vy  ✓
//   result.y = col0.y·vx + col1.y·vy =  s·vx + c·vy  ✓
// θ=90° 대입: (1,0) → (0,1) 으로 이동 → CCW 확인됨.
static simd::float4x4 rotationZ(float rad) {{
    float c = cosf(rad), s = sinf(rad);
    return simd::float4x4(
        simd::float4{{  c, s, 0, 0 }},
        simd::float4{{ -s, c, 0, 0 }},
        simd::float4{{  0, 0, 1, 0 }},
        simd::float4{{  0, 0, 0, 1 }}
    );
}}

// 2D 직교 투영 행렬 — 순수 스케일만 사용 (평행 이동 없음).
// Metal NDC: x, y ∈ [-1, 1].  aspect = width / height.
//   가로 > 세로 (aspect >= 1): x를 1/aspect로 스케일 축소 → 삼각형 폭 보정
//   세로 > 가로 (aspect <  1): y를 aspect로 스케일 축소 → 삼각형 높이 보정
// 가시 영역이 항상 원점 대칭이므로 tx=ty=0; 전체 행렬이 대각 행렬로 단순화.
static simd::float4x4 ortho2D(float aspect) {{
    if (aspect >= 1.0f) {{
        // 가로가 더 길거나 정사각형 — x축을 1/aspect로 압축
        return simd::float4x4(
            simd::float4{{1.0f / aspect, 0.0f, 0.0f, 0.0f}},
            simd::float4{{0.0f,         1.0f, 0.0f, 0.0f}},
            simd::float4{{0.0f,         0.0f, 1.0f, 0.0f}},
            simd::float4{{0.0f,         0.0f, 0.0f, 1.0f}}
        );
    }} else {{
        // 세로가 더 길 때 — y축을 aspect로 압축
        return simd::float4x4(
            simd::float4{{1.0f,   0.0f,  0.0f, 0.0f}},
            simd::float4{{0.0f,   aspect, 0.0f, 0.0f}},
            simd::float4{{0.0f,   0.0f,  1.0f, 0.0f}},
            simd::float4{{0.0f,   0.0f,  0.0f, 1.0f}}
        );
    }}
}}

@implementation SalvationView {{
    id<MTLLibrary>             _library;
    id<MTLCommandQueue>        _commandQueue;
    id<MTLRenderPipelineState> _pipelineState;
    // [Fix 4] 2D 삼각형은 depth test 불필요 — _depthState 제거
    dispatch_semaphore_t       _semaphore;
    id<MTLBuffer>              _uniformBuffers[kUniformBufferCount];
    id<MTLBuffer>              _vertexBuffer;
    int              _uniformBufferIndex;
    CFTimeInterval   _startTime;    // [Fix 6] 시작 시각 — 프레임 독립 애니메이션
    float            _aspectRatio;  // [Fix 2+3] 화면 비율 보정용
}}

- (id)initWithFrame:(CGRect)frame {{
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    self = [super initWithFrame:frame device:device];
    if (self) {{ [self _setup]; }}
    return self;
}}

- (void)_setup {{
    self.colorPixelFormat = MTLPixelFormatBGRA8Unorm;
    // [Fix 4] 2D 삼각형 — depth/stencil 불필요. MTLPixelFormatInvalid(기본값) 유지.
    // 3D로 전환 시: self.depthStencilPixelFormat = MTLPixelFormatDepth32Float; 재활성화.
    self.delegate = self;

    // CommandQueue를 먼저 생성 — 버텍스 버퍼 blit에 필요
    _commandQueue = [self.device newCommandQueue];

    NSError* error = nil;
    // NSBundle 리소스 API 우선 → .app 번들 / CLI 양쪽에서 동작
    NSURL* libURL = [[NSBundle mainBundle] URLForResource:@"shaders" withExtension:@"metallib"];
    if (!libURL) {{
        libURL = [NSURL fileURLWithPath:@"{metallib_name}"];
    }}
    _library = [self.device newLibraryWithURL:libURL error:&error];
    if (!_library) {{
        // fallback: 디버그 빌드의 기본 라이브러리 시도
        _library = [self.device newDefaultLibrary];
    }}
    if (!_library) {{
        NSLog(@"[Salvation] 셰이더 라이브러리 로드 실패: %@", error);
        // [Fix 5] exit(1) → NSAlert: GUI 앱이 갑자기 꺼지는 대신 사용자에게 메시지 표시.
        dispatch_async(dispatch_get_main_queue(), ^{{
            NSAlert* alert = [[NSAlert alloc] init];
            alert.messageText     = @"셰이더 라이브러리 로드 실패";
            alert.informativeText = [NSString stringWithFormat:
                @"shaders.metallib를 찾을 수 없습니다.\n"
                @"'salvation build' 명령으로 먼저 셰이더를 컴파일하세요.\n\n%@",
                error ? error.localizedDescription : @"(상세 오류 없음)"];
            alert.alertStyle = NSAlertStyleCritical;
            [alert runModal];
            [NSApp terminate:nil];
        }});
        return;
    }}

    id<MTLFunction> vertFunc = [_library newFunctionWithName:@"{vert}"];
    id<MTLFunction> fragFunc = [_library newFunctionWithName:@"{frag}"];

    MTLVertexDescriptor* vertDesc = [MTLVertexDescriptor new];
{vert_desc}
    MTLRenderPipelineDescriptor* pipelineDesc = [MTLRenderPipelineDescriptor new];
    pipelineDesc.rasterSampleCount            = self.sampleCount;
    pipelineDesc.vertexFunction               = vertFunc;
    pipelineDesc.fragmentFunction             = fragFunc;
    pipelineDesc.vertexDescriptor             = vertDesc;
    pipelineDesc.colorAttachments[0].pixelFormat = self.colorPixelFormat;
    // [Fix 4] depth/stencil attachment 없음 (2D 전용)
    _pipelineState = [self.device newRenderPipelineStateWithDescriptor:pipelineDesc error:&error];
    if (!_pipelineState) {{
        NSLog(@"[Salvation] 파이프라인 생성 실패: %@", error);
        dispatch_async(dispatch_get_main_queue(), ^{{
            NSAlert* alert = [[NSAlert alloc] init];
            alert.messageText     = @"렌더 파이프라인 생성 실패";
            alert.informativeText = [NSString stringWithFormat:@"%@",
                error ? error.localizedDescription : @"(상세 오류 없음)"];
            alert.alertStyle = NSAlertStyleCritical;
            [alert runModal];
            [NSApp terminate:nil];
        }});
        return;
    }}

    // 정적 삼각형 3개(48바이트) — Shared 모드로 충분.
    // Private + blit + waitUntilCompleted 패턴은 대형 지오메트리(수 MB↑)에 유리하지만
    // 이 규모에서는 오히려 startup latency만 추가하므로 Shared로 단순화.
    Vertex verts[] = {{
        {{ {{-0.5f, -0.5f, 0.0f, 1.0f}}, {{1.0f, 0.0f, 0.0f, 1.0f}} }},
        {{ {{ 0.0f,  0.5f, 0.0f, 1.0f}}, {{0.0f, 1.0f, 0.0f, 1.0f}} }},
        {{ {{ 0.5f, -0.5f, 0.0f, 1.0f}}, {{0.0f, 0.0f, 1.0f, 1.0f}} }},
    }};
    _vertexBuffer = [self.device newBufferWithBytes:verts
                                             length:sizeof(verts)
                                            options:MTLResourceStorageModeShared];

    for (int i = 0; i < kUniformBufferCount; i++) {{
        // StorageModeShared + WriteCombined: CPU 쓰기 / GPU 읽기 전용 버퍼
        _uniformBuffers[i] = [self.device
                               newBufferWithLength:sizeof({uniform_struct})
                               options:MTLResourceStorageModeShared |
                                       MTLResourceCPUCacheModeWriteCombined];
    }}

    _semaphore          = dispatch_semaphore_create(kUniformBufferCount);
    _uniformBufferIndex = 0;
    // [Fix 6] 프레임 카운터 대신 절대 시각 기준 애니메이션.
    // _frame 기반(_frame * 0.01f)은 화면 주사율(60/120Hz)에 속도가 묶여 버리는 문제가 있었음.
    _startTime = CACurrentMediaTime();
    // [Fix 8] drawableSize로 초기 aspect 계산 — Retina 배율 등으로 실제 크기가 512x512와 다를 수 있음.
    // drawableSizeWillChange:는 첫 프레임 이전에 항상 호출되지 않으므로 여기서 직접 초기화.
    CGSize sz = self.drawableSize;
    _aspectRatio = (sz.height > 0.0) ? (float)(sz.width / sz.height) : 1.0f;
    // _commandQueue: 위에서 이미 초기화
}}

// [Fix 3-2] drawRect: 제거 → MTKViewDelegate의 drawInMTKView: 사용
- (void)drawInMTKView:(MTKView*)view {{
    dispatch_semaphore_wait(_semaphore, DISPATCH_TIME_FOREVER);

    // [Fix 2-1] 창 최소화·리사이징 과도기에 nil이 반환될 수 있음.
    // nil 체크 없이 진행하면 Metal API Validation 크래시 발생.
    // nil일 때 세마포어를 해제하지 않으면 영구 데드락에 빠짐.
    MTLRenderPassDescriptor* rpd = view.currentRenderPassDescriptor;
    if (!rpd) {{
        dispatch_semaphore_signal(_semaphore);
        return;
    }}

    // [Fix 6] 경과 시간(초) 기반 각도 — 주사율(60/120Hz) 무관, 항상 일정 속도
    float elapsed = (float)(CACurrentMediaTime() - _startTime);
    // [Fix 1] column-major 교정된 rotationZ() 사용
    // [Fix 2+3] ortho2D()로 화면 비율 보정 후 행렬 합성
    simd::float4x4 model = rotationZ(elapsed);
    simd::float4x4 proj  = ortho2D(_aspectRatio);
    {uniform_struct}* uniforms = ({uniform_struct}*)[_uniformBuffers[_uniformBufferIndex] contents];
    uniforms->projectionViewModel = proj * model;

    id<MTLCommandBuffer>        cmd     = [_commandQueue commandBuffer];
    id<MTLRenderCommandEncoder> encoder = [cmd renderCommandEncoderWithDescriptor:rpd];

    // [Fix 4] depth state 없음 (2D — depthStencilPixelFormat = Invalid)
    [encoder setRenderPipelineState:_pipelineState];
    [encoder setVertexBuffer:_uniformBuffers[_uniformBufferIndex]
                      offset:0 atIndex:FrameUniformBuffer];
    [encoder setVertexBuffer:_vertexBuffer offset:0 atIndex:MeshVertexBuffer];
    [encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
    [encoder endEncoding];

    // [Fix 1] __block 제거: 세마포어 포인터를 덮어쓸 일이 없으므로 단순 값 복사로 캡처.
    // __block은 매 프레임 힙 할당을 유발해 초당 60회+ 루프에서 메모리 오버헤드가 발생함.
    dispatch_semaphore_t sem = _semaphore;
    [cmd addCompletedHandler:^(id<MTLCommandBuffer> _) {{
        dispatch_semaphore_signal(sem);
    }}];
    id<MTLDrawable> drawable = view.currentDrawable;
    if (drawable) {{
        [cmd presentDrawable:drawable];
    }}
    [cmd commit];
    // triple-buffering 인덱스 순환: 0→1→2→0
    _uniformBufferIndex = (_uniformBufferIndex + 1) % kUniformBufferCount;
}}

// [Fix 3] 화면 크기 변경 시 aspect ratio 갱신 → 다음 프레임 투영에 즉시 반영
- (void)mtkView:(MTKView*)view drawableSizeWillChange:(CGSize)size {{
    if (size.height > 0.0)
        _aspectRatio = (float)(size.width / size.height);
}}

@end
"#,
        metallib_name  = metallib_name,
        vert           = vert,
        frag           = frag,
        vert_desc      = vert_desc,
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

// shader_types.h Metal 셰이더 측 타입 — metal_stdlib 기준
fn emit_type_metal(ty: &Type) -> &'static str {
    match ty {
        Type::Bool    => "bool",
        Type::Int     => "int",
        Type::Uint    => "uint",
        Type::Float   => "float",
        Type::Float2  => "float2",
        Type::Float3  => "float3",
        Type::Float4  => "float4",
        Type::Mat2x2  => "float2x2",
        Type::Mat3x3  => "float3x3",
        Type::Mat4x4  => "float4x4",
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