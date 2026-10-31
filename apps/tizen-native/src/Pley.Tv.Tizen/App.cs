using Pley.Tv.Core;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Pley.Tv.Tizen.Screens;
using Tizen.NUI;

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
        Text.LoadFonts();
        window.BackgroundColor = PleyColors.Bg;
        gate = new KeyRepeatGate();
        shell = new Shell(window, gate, Exit);
        images = new Images(Current.DirectoryInfo.Cache);
        state = new AppState(Platform.Of());
        // Pictures come through the API in use, with its token.
        state.Api.Watch(api => images.Api = api);
        // The server ended the sign-in (revoked, or the refresh token spent): sign in again.
        state.SignedOut.Changed += signedOut =>
        {
            if (signedOut && state.Server.Value != null) Go(new PageKey.Link(state.Server.Value));
        };
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
            case PageKey.Link link:
                var model = new LinkModel(new AppLinkSource(state), new ContextScheduler(), link.Server);
                model.Done += () => Go(new PageKey.Home());
                var screen = new LinkScreen(model, ChangeServer);
                shell.Show(screen);
                screen.Started();
                break;
            case PageKey.Profiles:
                var profiles = new ProfilesModel(new AppProfilesSource(state), gate: !state.ProfileGatePassed);
                profiles.Done += () =>
                {
                    state.ProfileGatePassed = true;
                    Go(new PageKey.Home());
                };
                profiles.SignedOut += () => Go(new PageKey.Link(state.Server.Value ?? ""));
                shell.Show(new ProfilesScreen(shell, profiles, images, Approval));
                _ = profiles.Load();
                break;
            default:
                ShowHome();
                break;
        }
    }

    /// <summary>Home, with the rail; what was saved shows at once, and the server is asked for the rest.</summary>
    void ShowHome()
    {
        TvShell? tv = null;
        tv = new TvShell(state,
            item =>
            {
                if (item.Slot == RailSlot.Home) tv!.Restart(Home(tv));
                else if (tv!.PageOf?.Invoke(item.Key) is { } page) tv.Restart(page);
                else tv.Toast($"{item.Label} comes next in the native app.");
            },
            () => Go(new PageKey.Profiles()));
        tv.PageOf = key => key switch
        {
            PageKey.Item item => new ItemScreen(tv, shell, new ItemModel(new AppItemSource(state), item.ItemId, new ContextScheduler()), images, gate, k => Go(tv, k)),
            PageKey.Home => Home(tv),
            PageKey.Libraries => new LibrariesScreen(tv, shell, new LibrariesModel(new AppLibrarySource(state), new ContextScheduler()), gate, k => Go(tv, k)),
            PageKey.Library library => new LibraryScreen(tv, shell, new LibraryModel(new AppLibrarySource(state), library, new ContextScheduler()), images, gate, k => Go(tv, k)),
            PageKey.Playlists => new PlaylistsScreen(tv, new PlaylistsModel(new AppPlaylistSource(state), new ContextScheduler()), images, gate, k => Go(tv, k)),
            PageKey.Playlist playlist => new PlaylistScreen(tv, shell, new PlaylistModel(new AppPlaylistSource(state), playlist.PlaylistId, new ContextScheduler()), images, gate, k => Go(tv, k)),
            _ => null,
        };
        // Back from the player: Home asks again, for the new resume point and Continue Watching.
        tv.Play = request => new PlayerScreen(shell, state, request, gate, () => _ = state.LoadHome()).Show();
        tv.Actions = (id, continueWatching) => TileActions.Open(shell, state, id, continueWatching, r => tv.Play?.Invoke(r), k => Go(tv, k), tv.Toast);
        shell.Show(tv);
        tv.Restart(Home(tv));
        _ = state.LoadAccount();
        _ = state.LoadHome();
    }

    HomeScreen Home(TvShell tv) => new HomeScreen(tv, new HomeModel(state, new ContextScheduler()), images, gate, item => Go(tv, new PageKey.Item(item.Id)));

    /// <summary>Opens a page from another: an item's, over the one it was opened from; one the app doesn't have yet says so.</summary>
    void Go(TvShell tv, PageKey key)
    {
        if (!tv.Open(key)) tv.Toast("That page comes next in the native app.");
    }

    /// <summary>A code for a phone to approve: a profile that needs its password, or (id 0) any profile.</summary>
    LinkScreen Approval(ProfileTile profile)
    {
        var model = new LinkModel(new AppLinkSource(state, profile.Id), new ContextScheduler(), state.Server.Value ?? "");
        model.Done += () => Go(new PageKey.Home());
        LinkScreen? screen = null;
        screen = profile.Id == 0
            ? new LinkScreen(model, () => shell.Pop(screen!), "Sign in as another profile", "Approve this code on a phone signed in to Pley, and pick the profile there.", "Use another profile")
            : new LinkScreen(model, () => shell.Pop(screen!), "Approve on your phone",
                $"{profile.Name}'s profile needs its password: approve this code on a phone signed in to Pley, and pick {profile.Name} there.", "Use another profile");
        return screen;
    }

    void ChangeServer()
    {
        state.ForgetServer();
        Go(new PageKey.Servers());
    }

    static void Main(string[] args) => new App().Run(args);
}
