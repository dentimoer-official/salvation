#import <Foundation/Foundation.h>
#include <vector>        // C++ STL 사용 가능
#include <string>
#include "ffi_manager.h"

// C++ 코드와 Objective-C를 같이 쓸 수 있음
int salvation_add(int a, int b) {
    std::vector<int> v = {a, b}; // C++ STL 사용 예시
    return v[0] + v[1];
}

void salvation_log(const char* message) {
    @autoreleasepool {
        std::string cpp_msg(message); // C++ string으로 처리 가능
        NSString* msg = [NSString stringWithUTF8String:cpp_msg.c_str()];
        NSLog(@"[Salvation C++] %@", msg);
    }
}
