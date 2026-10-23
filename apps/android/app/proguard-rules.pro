# The web app calls the player bridge by name (window.PleyAndroid.play).
-keepclassmembers class app.pley.android.PlayerBridge {
    @android.webkit.JavascriptInterface <methods>;
}
