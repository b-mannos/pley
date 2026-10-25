// Signing in: the server's own page in the browser (phones), a code (TVs),
// the tokens and where they may go.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.auth"
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
}
