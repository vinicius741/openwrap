#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        [NEProvider startSystemExtensionMode];
    }
    return 0;
}
