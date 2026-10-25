// What the server sends and takes, as plain Kotlin for the JVM: the API's
// answers, server events, the capability report, formatting.
plugins {
    alias(libs.plugins.pley.jvm.library)
    alias(libs.plugins.kotlin.serialization)
}

dependencies {
    api(libs.kotlinx.serialization.json)
}
