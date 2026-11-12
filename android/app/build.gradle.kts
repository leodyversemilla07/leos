import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

/**
 * The Rust core is built for every Android ABI the app ships.
 *
 * The mapping is the usual one: the Gradle ABI name and the Rust target
 * name for the same architecture.
 */
val leosAbis = mapOf(
    "arm64-v8a" to "aarch64-linux-android",
    "armeabi-v7a" to "armv7-linux-androideabi",
    "x86_64" to "x86_64-linux-android",
)

/** The C compiler the NDK provides for each target. */
val leosCCompilers = mapOf(
    "aarch64-linux-android" to "aarch64-linux-android24-clang",
    "armv7-linux-androideabi" to "armv7a-linux-androideabi24-clang",
    "x86_64-linux-android" to "x86_64-linux-android24-clang",
)

val workspaceRoot = rootProject.file("..")

/** Where the Android SDK is. */
val sdkDir: String = run {
    val properties = Properties().apply {
        val file = rootProject.file("local.properties")
        if (file.exists()) {
            file.inputStream().use { load(it) }
        }
    }
    properties.getProperty("sdk.dir")
        ?: System.getenv("ANDROID_HOME")
        ?: System.getenv("ANDROID_SDK_ROOT")
        ?: ""
}

/** The NDK toolchain, which supplies the linker and the C compiler. */
val ndkBin: String = run {
    val ndkHome = System.getenv("ANDROID_NDK_HOME")
        ?: File(sdkDir, "ndk")
            .listFiles()
            ?.filter { it.isDirectory }
            ?.maxByOrNull { it.name }
            ?.absolutePath
        ?: ""
    val host = listOf("linux-x86_64", "darwin-x86_64", "darwin-arm64")
        .map { File(ndkHome, "toolchains/llvm/prebuilt/$it") }
        .firstOrNull { it.isDirectory }
        ?.absolutePath
        ?: ""
    File(host, "bin").absolutePath
}

/** Builds the Rust core and drops the shared libraries into the APK. */
val buildLeosCore = tasks.register("buildLeosCore") {
    group = "build"
    description = "Builds the Leos Rust core for every Android ABI."

    doLast {
        leosAbis.forEach { (abi, rustTarget) ->
            val compiler = File(ndkBin, leosCCompilers.getValue(rustTarget)).absolutePath
            exec {
                workingDir = workspaceRoot
                // The crypto library needs a C compiler, and the linker needs
                // the NDK wrapper, so both are pointed at explicitly.
                environment("PATH", "$ndkBin:${System.getenv("PATH")}")
                environment("CC_$rustTarget", compiler)
                environment("AR_$rustTarget", File(ndkBin, "llvm-ar").absolutePath)
                environment("RANLIB_$rustTarget", File(ndkBin, "llvm-ranlib").absolutePath)
                commandLine(
                    "cargo", "build",
                    "--release",
                    "--package", "leos-ffi",
                    "--target", rustTarget,
                )
            }
            copy {
                from(workspaceRoot.resolve("target/$rustTarget/release/libleos_ffi.so"))
                into(layout.buildDirectory.dir("jniLibs/$abi"))
            }
        }
    }
}

/** Reads the API address the app talks to, from local.properties or the environment. */
val apiBaseUrl: String = run {
    val local = Properties().apply {
        val file = rootProject.file("local.properties")
        if (file.exists()) {
            file.inputStream().use { load(it) }
        }
    }
    local.getProperty("leos.api.baseUrl")
        ?: System.getenv("LEOS_API_BASE_URL")
        ?: "http://10.0.2.2:8080"
}

android {
    namespace = "com.leos.wallet"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.leos.wallet"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        buildConfigField("String", "API_BASE_URL", "\"$apiBaseUrl\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        debug {
            applicationIdSuffix = ".debug"
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir(layout.buildDirectory.dir("jniLibs"))
        }
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

tasks.named("preBuild") {
    dependsOn(buildLeosCore)
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.6")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.6")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.6")
    implementation("androidx.activity:activity-compose:1.9.2")

    val composeBom = platform("androidx.compose:compose-bom:2024.09.03")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")

    implementation("androidx.navigation:navigation-compose:2.8.3")
    implementation("androidx.biometric:biometric:1.1.0")

    implementation("androidx.camera:camera-core:1.3.4")
    implementation("androidx.camera:camera-camera2:1.3.4")
    implementation("androidx.camera:camera-lifecycle:1.3.4")
    implementation("androidx.camera:camera-view:1.3.4")
    implementation("com.google.zxing:core:3.5.3")

    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
