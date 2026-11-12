# The Rust core is reached through JNI, so its entry points must survive
# shrinking and obfuscation.
-keepclasseswithmembernames class com.leos.wallet.RustCore {
    native <methods>;
}

# Keep the classes the JNI bridge looks up by name.
-keep class com.leos.wallet.RustCore { *; }
