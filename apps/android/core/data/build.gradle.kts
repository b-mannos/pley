// What the screens read: repositories over the API with snapshots on disk,
// the servers, the session, settings and live updates.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.data"
}

dependencies {
    api(project(":core:network"))
    api(project(":core:auth"))
    api(project(":core:model"))
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.lifecycle.process)

    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
}
