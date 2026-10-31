using System;
using System.Linq;
using Pley.Tv.Core;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Pley.Tv.Tizen.Screens;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen;

/// <summary>Pley for Samsung TV, native: a NUI window the remote drives.</summary>
class App : NUIApplication
{
    Shell shell = null!;
    AppState state = null!;
    Images images = null!;
    KeyRepeatGate gate = null!;

    protected override void OnCreate()
    {
        base.OnCreate();
        var window = Window.Instance;
        Ui.Measure(window);
        window.BackgroundColor = PleyColors.Bg;
        gate = new KeyRepeatGate();
        shell = new Shell(window, gate, Exit);
        images = new Images(Current.DirectoryInfo.Cache);
        state = new AppState(Platform.Of());
        Go(state.Start() switch
        {
            StartPage.Servers => new PageKey.Servers(),
            StartPage.Link => new PageKey.Link(state.Server.Value ?? ""),
            StartPage.Profiles => new PageKey.Profiles(),
            _ => new PageKey.Home(),
        });
    }

    /// <summary>Shows the page for <paramref name="key"/>, in place of what is on screen.</summary>
    void Go(PageKey key)
    {
        switch (key)
        {
            case PageKey.Servers:
                var servers = new ServersScreen(shell, new ServersModel(new AppServersSource(state), new ContextScheduler()))
                {
                    Chosen = chosen => Go(chosen.Next),
                };
                shell.Show(servers);
                break;
            case PageKey.Home:
                shell.Show(KitDemo());
                break;
            default:
                shell.Show(NotYet(key));
                break;
        }
    }

    /// <summary>A page the native app doesn't have yet: what comes there, and the way back to the server list.</summary>
    Sheet NotYet(PageKey key)
    {
        var page = new Sheet("not-yet");
        page.View.Add(Onboarding.BrandGlow());
        var server = state.Server.Value ?? "";
        var title = Text.Label(key is PageKey.Link ? "Sign in" : "Who's watching?", PleyType.PageTitle, PleyColors.Fg, width: 700);
        title.Position = new Position(Ui.Dp(96), Ui.Dp(27));
        var body = Text.Label($"{Pley.Tv.Core.Auth.Servers.HostPort(server)} is chosen. This screen comes next in the native app.", PleyType.Body, PleyColors.Muted, maxLines: 2, width: 600);
        body.Position = new Position(Ui.Dp(96), Ui.Dp(80));
        page.View.Add(title);
        page.View.Add(body);
        var row = new ButtonRow { Position = new Position(Ui.Dp(96), Ui.Dp(140)) };
        row.Add(new ButtonView("Change server", ButtonKind.Text, ChangeServer));
        row.Add(new ButtonView("Show the kit", ButtonKind.Text, () => shell.Show(KitDemo())));
        page.View.Add(row);
        page.Root.Add(row.Group);
        page.Tree.Enter(row.Group);
        return page;
    }

    void ChangeServer()
    {
        state.ForgetServer();
        Go(new PageKey.Servers());
    }

    /// <summary>The kit on placeholder rows, while the screens are built: focus, rows, the page's scrolling.</summary>
    Sheet KitDemo()
    {
        var sheet = new Sheet("home", Axis.Row);
        var tree = sheet.Tree;
        var root = sheet.Root;
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
        var version = Text.Label($"Pley for Samsung TV {Platform.Version()} · the kit, on placeholder rows", PleyType.Body, PleyColors.Muted);
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
        sheet.View.Add(column);
        sheet.View.Add(title);
        sheet.View.Add(version);
        // The rows' fade under the rail: Bg to x = 56, clear from 72.
        sheet.View.Add(new View
        {
            Size = new Size(Ui.Dp(PleyTv.RailFadeClearFrom), Ui.Dp(PleyTv.CanvasHeight)),
            Background = Gradients.Horizontal(PleyColors.Bg, new Color(PleyColors.Bg.R, PleyColors.Bg.G, PleyColors.Bg.B, 0), PleyTv.RailFadeSolidUntil / PleyTv.RailFadeClearFrom),
        });
        sheet.View.Add(railView);
        tree.Enter(page);
        // Back on the kit: to the server list, while there is no Home to go back to.
        sheet.Back = () =>
        {
            ChangeServer();
            return true;
        };
        return sheet;
    }

    static void Main(string[] args) => new App().Run(args);
}
