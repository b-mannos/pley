pluginManagement {
    // pley's convention plugins (pley.android.application and the rest).
    includeBuild("build-logic")
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "pley"

// One app for phones, tablets and TVs (docs/ANDROID.md has the map).
include(":app")
include(":core:common")
include(":core:model")
include(":core:auth")
include(":core:network")
include(":core:data")
include(":core:caps")
include(":core:player")
include(":core:designsystem")
include(":core:screens")
include(":core:testing")
include(":ui:phone")
include(":ui:tv")
include(":testing:fixtures")
include(":benchmark")
