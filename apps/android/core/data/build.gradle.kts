// What the screens read: repositories over the API with snapshots on disk,
// the servers, the session, settings and live updates.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.data"
    defaultConfig.testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    // DataStore traces through android.util.Log, a stub in plain unit tests.
    testOptions.unitTests.isReturnDefaultValues = true
}

dependencies {
    api(project(":core:network"))
    api(project(":core:auth"))
    api(project(":core:model"))
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.lifecycle.process)

    // FakePleyApi, FakeAuthGraph and the JSON fixtures.
    testImplementation(project(":core:testing"))
    // A server that signs in for real: tokens it refuses, a server that restarts.
    testImplementation(project(":testing:fixtures"))
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
    // The sign-in module's tokens are JSON (org.json is a stub in local unit tests).
    testImplementation(libs.json)
    // The web view app's SharedPreferences, as they are on a device.
    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.test.core)

    // Snapshots read under StrictMode, on a device.
    androidTestImplementation(project(":core:testing"))
    androidTestImplementation(libs.androidx.test.core)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.ext.junit)
}
