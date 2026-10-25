import app.pley.android.buildlogic.Sdk
import app.pley.android.buildlogic.configureAndroid
import app.pley.android.buildlogic.lib
import app.pley.android.buildlogic.libs
import com.android.build.api.dsl.ApplicationExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.gradle.kotlin.dsl.dependencies

/** `pley.android.application`: the app module (Kotlin built into AGP). */
class AndroidApplicationConventionPlugin : Plugin<Project> {
    override fun apply(target: Project): Unit = with(target) {
        pluginManager.apply("com.android.application")
        extensions.configure<ApplicationExtension> {
            configureAndroid(this)
            defaultConfig.targetSdk = Sdk.TARGET
        }
        dependencies {
            add("testImplementation", libs.lib("junit"))
        }
    }
}
