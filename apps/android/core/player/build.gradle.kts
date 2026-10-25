// Playing: the Media3 engine, the server session, subtitles, seek previews,
// markers, queues and music in the background.
plugins {
    alias(libs.plugins.pley.android.library)
    alias(libs.plugins.pley.android.compose)
}

android {
    namespace = "app.pley.android.player"
}

dependencies {
    api(project(":core:caps"))
    api(project(":core:data"))
    api(project(":core:network"))
    api(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.exoplayer.hls)
    implementation(libs.androidx.media3.datasource.okhttp)
    implementation(libs.androidx.media3.session)
    // SubtitleView only.
    implementation(libs.androidx.media3.ui)
    implementation(libs.androidx.media3.ui.compose)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.tracing.ktx)

    // The session's tests: fakes of the contracts, and the FixtureServer.
    testImplementation(project(":core:testing"))
    testImplementation(project(":testing:fixtures"))
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.media3.test.utils)
    testImplementation(libs.androidx.media3.test.utils.robolectric)
}
