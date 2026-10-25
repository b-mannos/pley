// FixtureServer: a fake pley server for tests, demos and benchmarks, so
// nothing writes into a real one. Debug and test builds only.
plugins {
    alias(libs.plugins.pley.jvm.library)
}

dependencies {
    api(project(":core:model"))
    api(libs.okhttp.mockwebserver3)
}
