// The TV interface (Android TV, Google TV, Fire TV): tv-material for the
// chrome, our own tiles, the rail, the D-pad rules and the TV player. Never
// Material 3 and never :ui:phone: the two themes conflict.
plugins {
    alias(libs.plugins.pley.android.library)
    alias(libs.plugins.pley.android.compose)
}

android {
    namespace = "app.pley.android.ui.tv"
}

dependencies {
    api(project(":core:screens"))
    implementation(project(":core:designsystem"))
    implementation(project(":core:player"))
    implementation(libs.androidx.tv.material)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.viewmodel.navigation3)
    implementation(libs.androidx.navigation3.ui)
    implementation(libs.androidx.media3.ui.compose)
    // The sign-in code's QR code on the link screen.
    implementation(libs.qrcodegen)

    testImplementation(libs.robolectric)
    testImplementation(libs.androidx.compose.ui.test.junit4)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}
