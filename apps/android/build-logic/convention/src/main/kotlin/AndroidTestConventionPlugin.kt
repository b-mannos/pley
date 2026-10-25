import app.pley.android.buildlogic.Sdk
import app.pley.android.buildlogic.configureAndroid
import com.android.build.api.dsl.TestExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure

/** `pley.android.test`: a module of instrumented tests run against the app (benchmarks). */
class AndroidTestConventionPlugin : Plugin<Project> {
    override fun apply(target: Project): Unit = with(target) {
        pluginManager.apply("com.android.test")
        extensions.configure<TestExtension> {
            configureAndroid(this)
            defaultConfig.targetSdk = Sdk.TARGET
        }
    }
}
