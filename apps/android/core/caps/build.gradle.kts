// What this device plays: decoders, Dolby Vision profiles, HDR types,
// display modes, passthrough, and the quirks of known models.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.caps"
}

dependencies {
    api(project(":core:common"))
    api(project(":core:model"))
    implementation(libs.androidx.media3.exoplayer)

    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.robolectric)
}
