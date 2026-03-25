#import <Foundation/Foundation.h>
#include "ffi_manager.h"

int salvation_add(int a, int b) {
    return a + b;
}

void salvation_log(const char* message) {
    @autoreleasepool {
        NSString* msg = [NSString stringWithUTF8String:message];
        NSLog(@"[Salvation] %@", msg);
    }
}
