plugins {
    alias(libs.plugins.pley.android.application)
    alias(libs.plugins.pley.android.compose)
    alias(libs.plugins.baselineprofile)
}

android {
    namespace = "app.pley.android"

    defaultConfig {
        applicationId = "app.pley.android"
        versionCode = 2
        versionName = "0.2.0"
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

// Benchmark builds too: the journeys walk the app against the demo server.
// Their build types come from the baselineprofile plugin, so their
// configurations exist only once it has added them.
configurations.matching { it.name == "benchmarkReleaseImplementation" || it.name == "nonMinifiedReleaseImplementation" }
    .configureEach { dependencies.add(project.dependencies.project(":testing:fixtures")) }

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
    implementation(libs.okhttp)
    implementation(libs.kotlinx.coroutines.android)
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
