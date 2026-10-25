// What this device plays: decoders, Dolby Vision profiles, HDR types,
// display modes, passthrough, and the quirks of known models.
plugins {
    alias(libs.plugins.pley.android.library)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "app.pley.android.caps"
    defaultConfig {
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
}

dependencies {
    api(project(":core:common"))
    api(project(":core:model"))
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.core.ktx)
    implementation(libs.kotlinx.serialization.json)

    testImplementation(project(":core:testing"))
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)

    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.kotlinx.coroutines.test)
}
