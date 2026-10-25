// One ViewModel per screen, shared by the phone and TV interfaces, and the
// routes between screens. No interface code.
plugins {
    alias(libs.plugins.pley.android.library)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "app.pley.android.screens"
}

dependencies {
    api(project(":core:data"))
    api(project(":core:player"))
    api(project(":core:caps"))
    api(project(":core:auth"))
    api(libs.androidx.lifecycle.viewmodel.ktx)
    api(libs.androidx.navigation3.runtime)
    implementation(libs.kotlinx.serialization.json)

    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.turbine)
}
