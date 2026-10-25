// For tests only: fakes of the contracts, JSON fixtures written from
// docs/API.md, and test rules. Never in the app.
plugins {
    alias(libs.plugins.pley.android.library)
}

android {
    namespace = "app.pley.android.testing"
}

dependencies {
    api(project(":core:model"))
    api(project(":core:common"))
    api(project(":core:auth"))
    api(project(":core:network"))
    api(project(":core:data"))
    api(project(":core:caps"))
    api(project(":core:player"))
    api(project(":core:screens"))
    api(libs.junit)
    api(libs.kotlinx.coroutines.test)
    api(libs.turbine)
}
