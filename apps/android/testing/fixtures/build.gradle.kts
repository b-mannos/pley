// FixtureServer: a fake pley server for tests, demos and benchmarks, so
// nothing writes into a real one. Debug and test builds only: the app takes
// it with debugImplementation, so no release carries its classes or media.
plugins {
    alias(libs.plugins.pley.jvm.library)
    alias(libs.plugins.kotlin.serialization)
}

dependencies {
    api(project(":core:model"))
    api(libs.okhttp.mockwebserver3)
}
