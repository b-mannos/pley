// Macrobenchmarks and the Baseline and Startup Profile generators, run
// against the app on a device or emulator (docs/ANDROID.md).
plugins {
    alias(libs.plugins.pley.android.test)
    alias(libs.plugins.baselineprofile)
}

android {
    namespace = "app.pley.android.benchmark"
    targetProjectPath = ":app"

    defaultConfig {
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // The profiles are made on the store build; the direct one has the same code.
        missingDimensionStrategy("channel", "store")
    }
}

baselineProfile {
    // Profiles are generated on purpose (docs/ANDROID.md), never during a build.
    useConnectedDevices = true
}

dependencies {
    implementation(libs.androidx.test.ext.junit)
    implementation(libs.androidx.test.runner)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.androidx.benchmark.macro.junit4)
}
