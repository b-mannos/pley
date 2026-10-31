using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen;

/// <summary>Pley for Samsung TV, native: a NUI window the remote drives.</summary>
class App : NUIApplication
{
    FocusTree tree = null!;
    Input input = null!;

    protected override void OnCreate()
    {
        base.OnCreate();
        var window = Window.Instance;
        Ui.Measure(window);
        window.BackgroundColor = PleyColors.Bg;
        var gate = new KeyRepeatGate();
        var root = new FocusGroup(Axis.Row, "root");
        tree = new FocusTree(root);
        input = new Input(window, tree, gate) { Back = Exit };
        var images = new Images(Current.DirectoryInfo.Cache);
        KitDemo(window, root, gate, images);
    }

    /// <summary>The kit on placeholder rows, while the screens are built: focus, rows, the page's scrolling.</summary>
    void KitDemo(Window window, FocusGroup root, KeyRepeatGate gate, Images images)
    {
        TextLabel? said = null;
        var railView = new RailView(tree, item => said!.Text = $"Rail: {item.Label}", () => said!.Text = "Rail: Who's watching?");
        root.Add(railView.Group);
        var demoLibraries = new[]
        {
            new Pley.Tv.Core.Api.Library { Id = 1, Name = "Movies", Kind = "movies" },
            new Pley.Tv.Core.Api.Library { Id = 2, Name = "TV Shows", Kind = "shows" },
            new Pley.Tv.Core.Api.Library { Id = 3, Name = "Music", Kind = "music" },
        };
        railView.SetItems(Rail.Items(demoLibraries, liveTv: true));
        railView.SetProfile(new RailProfile("Anna", new Color(0.878f, 0.478f, 0.373f, 1)));
        railView.SetCurrent("home");

        var page = root.Add(new FocusGroup(Axis.Column, "page"));
        var title = Text.Label("Pley", PleyType.PageTitle, PleyColors.Fg);
        title.Position = new Position(Ui.Dp(PleyTv.Gutter), Ui.Dp(PleyTv.SafeVertical));
        var version = Text.Label($"Pley for Samsung TV {VersionShown()} · the kit, on placeholder rows", PleyType.Body, PleyColors.Muted);
        version.Position = new Position(Ui.Dp(PleyTv.Gutter), Ui.Dp(PleyTv.SafeVertical + 40));
        said = version;
        var column = new RowColumnView(gate, top: 110);
        bool ToRail() => railView.FocusCurrent();
        var kinds = new[] { ("Continue Watching", TileKind.Landscape, 12), ("Recently added movies", TileKind.Poster, 40), ("Recently added shows", TileKind.Poster, 25), ("Cast", TileKind.Person, 14), ("Episodes", TileKind.Episode, 10), ("A short row", TileKind.Poster, 3) };
        var id = 0;
        foreach (var (name, kind, count) in kinds)
        {
            var models = Enumerable.Range(1, count).Select(n => new TileModel(++id, $"{name} {n}", n % 3 == 0 ? "2024" : "S1 · E" + n,
                Array.Empty<string>(), kind == TileKind.Landscape ? n / (float)count : 0, n % 5 == 0, kind == TileKind.Poster && n % 4 == 0 ? n : 0,
                n % 6 == 0 ? new[] { "4K", "DV" } : Array.Empty<string>(), $"{name} {n}")).ToList();
            column.Add(new RowView(name, models, kind, gate, images, m => version.Text = $"Opened {m.Title}", m => version.Text = $"Actions of {m.Title}", ToRail));
        }
        page.Add(column.Group);
        window.Add(column);
        window.Add(title);
        window.Add(version);
        // The rows' fade under the rail: Bg to x = 56, clear from 72.
        window.Add(new View
        {
            Size = new Size(Ui.Dp(PleyTv.RailFadeClearFrom), Ui.Dp(PleyTv.CanvasHeight)),
            Background = Fade(),
        });
        window.Add(railView);
        tree.Enter(page);
    }

    static PropertyMap Fade()
    {
        var stops = new PropertyArray();
        stops.Add(new PropertyValue(0f));
        stops.Add(new PropertyValue(PleyTv.RailFadeSolidUntil / PleyTv.RailFadeClearFrom));
        stops.Add(new PropertyValue(1f));
        var colors = new PropertyArray();
        colors.Add(new PropertyValue(PleyColors.Bg));
        colors.Add(new PropertyValue(PleyColors.Bg));
        colors.Add(new PropertyValue(new Color(PleyColors.Bg.R, PleyColors.Bg.G, PleyColors.Bg.B, 0)));
        var map = new PropertyMap();
        map.Add(Visual.Property.Type, new PropertyValue((int)Visual.Type.Gradient));
        map.Add(GradientVisualProperty.StartPosition, new PropertyValue(new Vector2(-0.5f, 0)));
        map.Add(GradientVisualProperty.EndPosition, new PropertyValue(new Vector2(0.5f, 0)));
        map.Add(GradientVisualProperty.StopOffset, new PropertyValue(stops));
        map.Add(GradientVisualProperty.StopColor, new PropertyValue(colors));
        return map;
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

    static void Main(string[] args) => new App().Run(args);
}
