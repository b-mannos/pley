import app.pley.android.buildlogic.Sdk
import app.pley.android.buildlogic.configureAndroid
import app.pley.android.buildlogic.lib
import app.pley.android.buildlogic.libs
import com.android.build.api.dsl.LibraryExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.dependencies

/** `pley.android.library`: an Android library module of the app. */
class AndroidLibraryConventionPlugin : Plugin<Project> {
    override fun apply(target: Project): Unit = with(target) {
        pluginManager.apply("com.android.library")
        extensions.configure<LibraryExtension> {
            configureAndroid(this)
            lint.targetSdk = Sdk.TARGET
            testOptions.targetSdk = Sdk.TARGET
        }
        dependencies {
            add("testImplementation", libs.lib("junit"))
        }
    }
}
