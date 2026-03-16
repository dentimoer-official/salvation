#import <Metal/Metal.h>
#import <Foundation/Foundation.h>
#include <string.h>
#include <stdlib.h>

// extern "C" 절대 쓰지 말 것 — .m 파일은 Objective-C이므로
// 최상위에 선언한 C 함수는 자동으로 C ABI로 노출됨

bool slvt_metal_is_supported(void) {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    return device != nil;
}

// UTF8String은 NSString 내부 포인터라 Rust로 넘기면 수명이 위험함
// strdup()으로 힙 복사해서 안전하게 전달
char* slvt_metal_device_name(void) {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (device == nil) return NULL;
    const char* name = [device.name UTF8String];
    return strdup(name); // Rust 쪽에서 반드시 free 해줘야 함
}

uint64_t slvt_metal_recommended_max_working_set_size(void) {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (device == nil) return 0;
    return device.recommendedMaxWorkingSetSize;
}

bool slvt_metal_has_unified_memory(void) {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (device == nil) return false;
    return device.hasUnifiedMemory;
}