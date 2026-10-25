plugins {
    alias(libs.plugins.pley.android.application)
    alias(libs.plugins.pley.android.compose)
    alias(libs.plugins.baselineprofile)
}

android {
    namespace = "app.pley.android"

    defaultConfig {
        applicationId = "app.pley.android"
        versionCode = 1
        versionName = "0.1.0"
    }

    // Where a build goes: the stores (no self-updater), or the pley server's
    // own download for sideloading (with an update check).
    flavorDimensions += "channel"
    productFlavors {
        create("store") {
            dimension = "channel"
        }
        create("direct") {
            dimension = "channel"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    buildFeatures {
        buildConfig = true
    }
}

baselineProfile {
    // The profiles :benchmark generates, committed in src/main (docs/ANDROID.md).
    mergeIntoMain = true
    saveInSrc = true
    automaticGenerationDuringBuild = false
}

dependencies {
    implementation(project(":core:common"))
    implementation(project(":core:model"))
    implementation(project(":core:auth"))
    implementation(project(":core:network"))
    implementation(project(":core:data"))
    implementation(project(":core:caps"))
    implementation(project(":core:player"))
    implementation(project(":core:designsystem"))
    implementation(project(":core:screens"))
    implementation(project(":ui:phone"))
    implementation(project(":ui:tv"))
    baselineProfile(project(":benchmark"))
    // The fake server, for demos in debug builds; never in a release.
    debugImplementation(project(":testing:fixtures"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.profileinstaller)
    implementation(libs.androidx.tracing.ktx)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.exoplayer.hls)
    implementation(libs.androidx.media3.ui)
    // OkHttp sends the session token to the server only, also across redirects.
    implementation(libs.androidx.media3.datasource.okhttp)
    // The player bridge, offered to the server's own pages only.
    implementation(libs.androidx.webkit)
    // Signing in happens in the browser (a Custom Tab), never in a web view.
    implementation(libs.androidx.browser)
    implementation(libs.okhttp)
    implementation(libs.kotlinx.coroutines.android)

    // Android's org.json is a stub in local unit tests.
    testImplementation(libs.json)
    // A real server for the sign-in module's tests, over http and https.
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.okhttp.tls)
}

// One command for every module (CI and docs/ANDROID.md): the flavors' debug
// unit tests and lint under the names the other modules use.
tasks.register("testDebugUnitTest") {
    group = "verification"
    dependsOn("testStoreDebugUnitTest", "testDirectDebugUnitTest")
}
tasks.register("lintDebug") {
    group = "verification"
    dependsOn("lintStoreDebug", "lintDirectDebug")
}
