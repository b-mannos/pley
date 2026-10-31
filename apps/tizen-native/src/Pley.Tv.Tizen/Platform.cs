using System.Reflection;
using Pley.Tv.Core;
using Pley.Tv.Core.Auth;
using Tizen.Applications;

namespace Pley.Tv.Tizen;

/// <summary>What the app keeps between launches, in the app's own preferences.</summary>
sealed class PreferenceStore : IKeyValue
{
    public string? Get(string key) => Preference.Contains(key) ? Preference.Get<string>(key) : null;

    public void Set(string key, string value) => Preference.Set(key, value);

    public void Remove(string key)
    {
        if (Preference.Contains(key)) Preference.Remove(key);
    }
}

/// <summary>The TV as the shared state sees it: the app's name, version and client, and the TV's model.</summary>
static class Platform
{
    /// <summary>The Samsung apps' sign-in client: the native app signs in as the web one does.</summary>
    const string ClientId = "https://pley-media-server.github.io/pley/oauth/tizen.json";

    public static AppPlatform Of() => new AppPlatform("tizen", ClientId, new PreferenceStore())
    {
        Version = Version(),
        OsVersion = Info("http://tizen.org/feature/platform.version"),
        Model = Info("http://tizen.org/system/model_name"),
        TokenVault = new KeyManagerVault(),
    };

    /// <summary>The version people see: "0.0.1", without the commit.</summary>
    public static string Version()
    {
        var info = typeof(Platform).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "?";
        var plus = info.IndexOf('+');
        return plus < 0 ? info : info.Substring(0, plus);
    }

    static string? Info(string key) => global::Tizen.System.Information.TryGetValue(key, out string value) ? value : null;
}
