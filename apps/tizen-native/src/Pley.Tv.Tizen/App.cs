using System;
using System.Reflection;
using System.Runtime.InteropServices;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen;

/// <summary>Pley for Samsung TV, native: a NUI window the remote drives.</summary>
class App : NUIApplication
{
    protected override void OnCreate()
    {
        base.OnCreate();
        var window = Window.Instance;
        window.BackgroundColor = Theme.Background;
        window.KeyEvent += OnKey;
        window.Add(StartScreen(window));
    }

    /// <summary>The start screen: Pley, its version, and what the TV says it is.</summary>
    static View StartScreen(Window window)
    {
        global::Tizen.System.Information.TryGetValue("http://tizen.org/feature/platform.version", out string tizen);
        global::Tizen.System.Information.TryGetValue("http://tizen.org/system/model_name", out string model);
        var screen = new View
        {
            Size = new Size(window.Size.Width, window.Size.Height),
            Layout = new LinearLayout
            {
                LinearOrientation = LinearLayout.Orientation.Vertical,
                LinearAlignment = LinearLayout.Alignment.Center,
                CellPadding = new Size2D(0, 24),
            },
        };
        screen.Add(new TextLabel("Pley") { PointSize = 64, TextColor = Theme.Foreground });
        screen.Add(new TextLabel($"Pley for Samsung TV {VersionShown()}") { PointSize = 22, TextColor = Theme.Muted });
        screen.Add(new TextLabel($"{model} · Tizen {tizen} · {RuntimeInformation.FrameworkDescription}") { PointSize = 18, TextColor = Theme.Muted });
        return screen;
    }

    /// <summary>The version and the commit it was built from: "0.0.1 (1a2b3c4d5)".</summary>
    static string VersionShown()
    {
        var info = typeof(App).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "";
        var plus = info.IndexOf('+');
        if (plus < 0) return info;
        var commit = info.Substring(plus + 1);
        return $"{info.Substring(0, plus)} ({(commit.Length > 9 ? commit.Substring(0, 9) : commit)})";
    }

    void OnKey(object? sender, Window.KeyEventArgs e)
    {
        if (e.Key.State != Key.StateType.Down) return;
        // Back on the first screen leaves the app; Samsung's Exit key always does, by itself.
        if (e.Key.KeyPressedName == "XF86Back" || e.Key.KeyPressedName == "Escape") Exit();
    }

    static void Main(string[] args) => new App().Run(args);
}
