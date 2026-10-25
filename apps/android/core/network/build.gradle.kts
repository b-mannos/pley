// One OkHttp stack for the app, the typed API, server events, discovery and
// the network checks.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.network"
}

dependencies {
    api(project(":core:common"))
    api(project(":core:model"))
    api(project(":core:auth"))
    api(libs.okhttp)
    implementation(libs.okhttp.sse)
    implementation(libs.kotlinx.serialization.json.okio)
    implementation(libs.androidx.core.ktx)

    testImplementation(libs.okhttp.mockwebserver3)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
}
