// Playing: the Media3 engine, the server session, subtitles, seek previews,
// markers, queues and music in the background.
plugins {
    alias(libs.plugins.pley.android.library)
    alias(libs.plugins.pley.android.compose)
}

android {
    namespace = "app.pley.android.player"
    defaultConfig {
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
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
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.tracing.ktx)
    implementation(libs.kotlinx.coroutines.android)

    // Fakes of the contracts, and the FixtureServer with its media clips.
    testImplementation(project(":core:testing"))
    testImplementation(project(":testing:fixtures"))
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)
    testImplementation(libs.androidx.media3.test.utils)
    testImplementation(libs.androidx.media3.test.utils.robolectric)

    androidTestImplementation(project(":testing:fixtures"))
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.core)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.kotlinx.coroutines.test)
    androidTestImplementation(libs.okhttp.mockwebserver3)
    // An empty ComponentActivity to show the video in.
    androidTestImplementation(libs.androidx.compose.ui.test.manifest)
}
