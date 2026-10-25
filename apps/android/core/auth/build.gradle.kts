// Signing in: the server's own page in the browser (phones), a code (TVs),
// the tokens and where they may go.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.auth"
    defaultConfig.testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    // Robolectric's Android 16 reaches into the JDK (ApplicationSharedMemory).
    testOptions.unitTests.all { it.jvmArgs("--add-opens=java.base/jdk.internal.access=ALL-UNNAMED") }
}

dependencies {
    api(project(":core:common"))
    api(libs.okhttp)
    api(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.core.ktx)
    // The browser's Custom Tabs, never a web view.
    implementation(libs.androidx.browser)

    testImplementation(libs.json)
    // A real server over http and https for the tests.
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.okhttp.tls)
    testImplementation(libs.kotlinx.coroutines.test)
    // The Custom Tab's intent, on an Android of Robolectric's.
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)

    // The Keystore's vault, on a device.
    androidTestImplementation(libs.androidx.test.core)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.ext.junit)
}
