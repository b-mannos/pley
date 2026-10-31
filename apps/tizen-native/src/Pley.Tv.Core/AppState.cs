using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text.Json;
using System.Threading.Tasks;
using Pley.Tv.Core.Api;
using Pley.Tv.Core.Auth;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core;

/// <summary>What a person sets on this TV (Settings).</summary>
public sealed record Settings
{
    /// <summary>"small", "normal", "large" or "largest".</summary>
    public string SubtitleSize { get; init; } = "normal";
    public bool SubtitleBox { get; init; }
    public string SubtitleFont { get; init; } = "default";
    public string SubtitlePosition { get; init; } = "bottom";
    /// <summary>Up Next plays after its 10 s countdown.</summary>
    public bool AutoplayNext { get; init; } = true;
    public bool ReduceMotion { get; init; }
    /// <summary>The compact readout over the video while the controls are hidden.</summary>
    public bool PlaybackDetails { get; init; }
    /// <summary>The quality last picked ("auto", "original", "8m"…); Automatic until one is.</summary>
    public string Quality { get; init; } = "auto";
    /// <summary>Send TrueHD to the receiver as it is (the TV can't promise it).</summary>
    public bool PassthroughTrueHd { get; init; }
    /// <summary>Send DTS to the receiver as it is.</summary>
    public bool PassthroughDts { get; init; }
}

/// <summary>A server this TV signed in to or chose, newest first on the server list.</summary>
public sealed record SavedServer(string Url, string Name, long LastUsed);

/// <summary>The first screen, from what the TV kept (no network): the Android app's StartGate.</summary>
public enum StartPage
{
    Servers,
    Link,
    Profiles,
    Home,
}

/// <summary>What the TV app gives the shared state: its client id, its name, and where it keeps things.</summary>
public sealed record AppPlatform(string Name, string ClientId, IKeyValue Storage);

/// <summary>
/// The app's state: the server signed in to, its API and tokens, who is watching, the
/// libraries, the settings, Home as last seen (drawn first at the next launch), and the
/// server's events. The web TV app's <c>state.ts</c>; it lives on the UI thread, where its
/// awaits come back to.
/// </summary>
public sealed class AppState
{
    const string ServerKey = "pley.server";
    const string SettingsKey = "pley.settings";
    /// <summary>Set once a quality is picked: the settings saved before Automatic held Original whether picked or not.</summary>
    const string QualityPickedKey = "pley.qualityPicked";
    const string HomeKey = "pley.home";
    /// <summary>How many profiles the server's household has, as last heard; kept per server.</summary>
    const string HouseholdKey = "pley.household";
    /// <summary>Who is watching and their libraries, as last heard: the rail and the library pages before the server answers.</summary>
    const string MeKey = "pley.me";
    const string LibrariesKey = "pley.libraries";
    const string ServersKey = "pley.servers";
    /// <summary>The household's profiles, as last heard; kept per server.</summary>
    const string ProfilesKey = "pley.householdProfiles";
    /// <summary>How long an item's details are kept for pages and playbacks.</summary>
    const long ItemTtlMs = 5 * 60_000;
    /// <summary>Watch state changes come in bursts (an episode, its season and show; a play reaching 90 %): gathered this long.</summary>
    const long StateGatherMs = 300;
    /// <summary>How long "Showing what Pley had saved. Reconnecting…" shows: as long as an error's toast.</summary>
    const long OfflineNoticeMs = 6_000;
    /// <summary>While the server is out of reach it is asked this often (and given as long to answer).</summary>
    const int OfflineAskMs = 5_000;

    static readonly JsonSerializerOptions Camel = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    readonly AppPlatform platform;
    readonly Transport http;
    readonly EventConnect connect;
    readonly IScheduler scheduler;
    readonly IKeyValue store;
    readonly List<Action> leavers = new List<Action>();
    readonly List<Action<ServerEvent>> eventListeners = new List<Action<ServerEvent>>();
    readonly Dictionary<long, (ItemDetail Detail, long At)> items = new Dictionary<long, (ItemDetail, long)>();
    readonly LinkedList<long> itemOrder = new LinkedList<long>();
    readonly HashSet<long> stateIds = new HashSet<long>();
    readonly HashSet<long> stateParents = new HashSet<long>();
    Tokens? tokens;
    EventStream? events;
    IDisposable? libraryTimer, stateTimer, noticeTimer, awayTimer;
    /// <summary>Playbacks on top: Home and the pages wait to ask again until they end (their own reports change the watch state too).</summary>
    int holds;
    /// <summary>The libraries' order changes on their way to the server, one at a time.</summary>
    Task orderQueue = Task.CompletedTask;
    long lastSync;

    public AppState(AppPlatform platform, Transport? http = null, EventConnect? connect = null, IScheduler? scheduler = null)
    {
        this.platform = platform;
        this.http = http ?? Transports.Http;
        this.connect = connect ?? EventStream.Http;
        this.scheduler = scheduler ?? new ContextScheduler();
        store = platform.Storage;
        var kept = Read<Settings>(SettingsKey, Camel) ?? new Settings();
        // Original was the default before Automatic, and was saved along with any other setting: unless picked, it is Automatic now.
        if (kept.Quality == "original" && store.Get(QualityPickedKey) != "1") kept = kept with { Quality = "auto" };
        Settings = new Signal<Settings>(kept);
        var server = store.Get(ServerKey);
        if (!string.IsNullOrEmpty(server)) Use(server!);
        // What was saved is trusted only in shape: a store another build wrote, or a damaged one, is left out.
        Home.Value = Read<List<Hub>>(HomeKey);
        Me.Value = Read<User>(MeKey) is { Id: > 0 } me ? me : null;
        Libraries.Value = Read<List<Library>>(LibrariesKey);
    }

    public Signal<string?> Server { get; } = new Signal<string?>(null);
    public Signal<Api.Api?> Api { get; } = new Signal<Api.Api?>(null);
    public Signal<User?> Me { get; } = new Signal<User?>(null);
    public Signal<IReadOnlyList<Library>?> Libraries { get; } = new Signal<IReadOnlyList<Library>?>(null);
    public Signal<IReadOnlyList<HouseholdProfile>?> Profiles { get; } = new Signal<IReadOnlyList<HouseholdProfile>?>(null);
    public Signal<Settings> Settings { get; }
    public Signal<IReadOnlyList<Hub>?> Home { get; } = new Signal<IReadOnlyList<Hub>?>(null);
    public Signal<Exception?> HomeError { get; } = new Signal<Exception?>(null);
    /// <summary>The last try at who is watching and the libraries failed (and none were saved): the library pages offer Retry.</summary>
    public Signal<Exception?> AccountError { get; } = new Signal<Exception?>(null);
    /// <summary>Bumped by the server's events, so screens that list things fetch again.</summary>
    public Signal<int> Changes { get; } = new Signal<int>(0);
    /// <summary>The server signed the TV out: show sign-in.</summary>
    public Signal<bool> SignedOut { get; } = new Signal<bool>(false);
    /// <summary>Whether the TV shows the app (not another app, its home screen or standby).</summary>
    public Signal<bool> Visible { get; } = new Signal<bool>(true);
    /// <summary>
    /// The server answered a screen nothing (the Android app's TvOfflineState): what shows is what
    /// was saved. Raised by a load the server couldn't answer, lowered by its events connecting or
    /// a screen's fresh answer after a failure.
    /// </summary>
    public Signal<bool> Offline { get; } = new Signal<bool>(false);
    /// <summary>"Showing what Pley had saved. Reconnecting…" shows, for 6 s from going offline; then the rail's avatar keeps a mark.</summary>
    public Signal<bool> OfflineNotice { get; } = new Signal<bool>(false);
    /// <summary>
    /// "Who's watching?" was answered in this launch (a profile picked, a PIN checked, or a
    /// sign-in approved on a phone): the current profile's PIN isn't asked again until the app
    /// starts over.
    /// </summary>
    public bool ProfileGatePassed { get; set; }

    public Transport Http => http;
    public AppPlatform Platform => platform;
    public Tokens? TokenStore => tokens;
    public bool SignedIn => tokens?.SignedIn == true;

    T? Read<T>(string key, JsonSerializerOptions? options = null) where T : class
    {
        try
        {
            var text = store.Get(key);
            return string.IsNullOrEmpty(text) ? null : JsonSerializer.Deserialize<T>(text!, options ?? ApiJson.Options);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    void Write<T>(string key, T value, JsonSerializerOptions? options = null) => store.Set(key, JsonSerializer.Serialize(value, options ?? ApiJson.Options));

    /// <summary>The first screen, from what the TV kept.</summary>
    public StartPage Start()
    {
        if (Server.Value == null) return StartPage.Servers;
        if (tokens?.SignedIn != true) return StartPage.Link;
        return HouseholdSize() > 1 && AlwaysStartAs() == null ? StartPage.Profiles : StartPage.Home;
    }

    /// <summary>Signs in to <paramref name="server"/> from now on (its tokens, if the TV has them).</summary>
    public void Use(string server)
    {
        var home = Addresses.Origin(server);
        if (home == null) return;
        if (Server.Value == home && Api.Value != null) return;
        events?.Stop();
        events = null;
        if (Server.Value != null && Server.Value != home) LeaveServer();
        Server.Value = home;
        store.Set(ServerKey, home);
        tokens = new Tokens(home, platform.ClientId, store, http, _ =>
        {
            SignedOut.Value = true;
            events?.Stop();
        }, () => scheduler.Now);
        Api.Value = new Api.Api(home, tokens, http);
        SignedOut.Value = false;
    }

    /// <summary>The servers this TV chose, most recently used first.</summary>
    public IReadOnlyList<SavedServer> SavedServers() =>
        (Read<List<SavedServer>>(ServersKey, Camel) ?? new List<SavedServer>())
            .Where(s => s != null && !string.IsNullOrEmpty(s.Url)).OrderByDescending(s => s.LastUsed).ToList();

    /// <summary>Keeps <paramref name="url"/> on the server list, as used now.</summary>
    public void RememberServer(string url, string name)
    {
        var home = Addresses.Origin(url);
        if (home == null) return;
        var list = SavedServers().Where(s => s.Url != home).ToList();
        list.Insert(0, new SavedServer(home, name, scheduler.Now));
        Write(ServersKey, list.Take(20).ToList(), Camel);
    }

    /// <summary>Whether the TV has a sign-in for <paramref name="url"/>.</summary>
    public bool HasTokens(string url)
    {
        var home = Addresses.Origin(url);
        return home != null && new Tokens(home, platform.ClientId, store, http).SignedIn;
    }

    /// <summary>
    /// This TV's own id for its playbacks ("tizen-1f3a…"), kept for good: the server ends a
    /// client's earlier sessions when it starts another, so two TVs of a household must never share one.
    /// </summary>
    public string InstallId()
    {
        var id = store.Get("pley.installId");
        if (string.IsNullOrEmpty(id))
        {
            var bytes = new byte[16];
            using (var random = RandomNumberGenerator.Create()) random.GetBytes(bytes);
            id = $"{platform.Name}-{string.Concat(bytes.Select(b => b.ToString("x2")))}";
            store.Set("pley.installId", id);
        }
        return id!;
    }

    /// <summary>The profile "Always start as" names for this server; null asks who's watching.</summary>
    public long? AlwaysStartAs() => long.TryParse(store.Get($"pley.alwaysStartAs.{Server.Value ?? ""}"), out var id) ? id : null;

    public void SetAlwaysStartAs(long? id)
    {
        var key = $"pley.alwaysStartAs.{Server.Value ?? ""}";
        if (id == null) store.Remove(key);
        else store.Set(key, id.Value.ToString(System.Globalization.CultureInfo.InvariantCulture));
    }

    /// <summary>How many profiles the household had when last asked (1 while unknown).</summary>
    public int HouseholdSize() => int.TryParse(store.Get($"{HouseholdKey}.{Server.Value ?? ""}"), out var n) && n > 0 ? n : 1;

    /// <summary>The household as last fetched, for "Who's watching?" before the server answers.</summary>
    public IReadOnlyList<HouseholdProfile> SavedHousehold() => Read<List<HouseholdProfile>>($"{ProfilesKey}.{Server.Value ?? ""}") ?? new List<HouseholdProfile>();

    public void KeepHousehold(IReadOnlyList<HouseholdProfile> profiles)
    {
        Profiles.Value = profiles;
        var server = Server.Value ?? "";
        store.Set($"{HouseholdKey}.{server}", profiles.Count.ToString(System.Globalization.CultureInfo.InvariantCulture));
        Write($"{ProfilesKey}.{server}", profiles);
    }

    /// <summary><paramref name="fn"/> runs when what plays must end: a profile switch, signing out, another server, the app closing.</summary>
    public void OnLeave(Action fn) => leavers.Add(fn);

    /// <summary>Ends what plays in the background before the profile, the sign-in, the server or the app goes.</summary>
    public void EndPlayback()
    {
        foreach (var fn in leavers.ToList()) fn();
    }

    /// <summary>The app acts as another profile now: what was shown for the one before goes.</summary>
    public async Task Switched()
    {
        EndPlayback();
        ForgetAccount();
        Home.Value = null;
        HomeError.Value = null;
        ForgetItems();
        Me.Value = null;
        await LoadAccount();
        _ = LoadHome();
    }

    /// <summary>Forgets a server of the list: its sign-in ends, and what was saved from it goes.</summary>
    public async Task Forget(string url)
    {
        var home = Addresses.Origin(url);
        if (home == null) return;
        Write(ServersKey, SavedServers().Where(s => s.Url != home).ToList(), Camel);
        var those = home == Server.Value && tokens != null ? tokens : new Tokens(home, platform.ClientId, store, http);
        if (home == Server.Value) ForgetServer();
        try
        {
            await those.SignOut();
        }
        catch (Exception)
        {
            // Forgotten here whatever the server says.
        }
    }

    /// <summary>Forgets the server in use (Change server).</summary>
    public void ForgetServer()
    {
        EndPlayback();
        events?.Stop();
        events = null;
        store.Remove(ServerKey);
        var server = Server.Value ?? "";
        store.Remove($"{HouseholdKey}.{server}");
        store.Remove($"{ProfilesKey}.{server}");
        LeaveServer();
        Server.Value = null;
        Api.Value = null;
        tokens = null;
    }

    /// <summary>What was shown from the server in use goes: another one comes, or none.</summary>
    void LeaveServer()
    {
        EndPlayback();
        ForgetAccount();
        ForgetItems();
        Home.Value = null;
        HomeError.Value = null;
        AccountError.Value = null;
        Me.Value = null;
        Libraries.Value = null;
        Profiles.Value = null;
        Offline.Value = false;
        OfflineNotice.Value = false;
        ProfileGatePassed = false;
    }

    public async Task SignOutNow()
    {
        events?.Stop();
        // What plays ends while the sign-in that made it still works.
        EndPlayback();
        if (tokens != null) await tokens.SignOut();
        ForgetAccount();
        Home.Value = null;
        SignedOut.Value = true;
    }

    public void UpdateSettings(Func<Settings, Settings> change)
    {
        var before = Settings.Value;
        Settings.Value = change(before);
        Write(SettingsKey, Settings.Value, Camel);
        if (Settings.Value.Quality != before.Quality) store.Set(QualityPickedKey, "1");
    }

    // ---- what the server has

    /// <summary>What was saved of the profile signed in (Home, who it is, its libraries): another profile, sign-in or server comes.</summary>
    void ForgetAccount()
    {
        store.Remove(HomeKey);
        store.Remove(MeKey);
        store.Remove(LibrariesKey);
    }

    /// <summary>
    /// Who is watching, the libraries and the household, after sign-in or a profile switch; kept,
    /// so the rail and the library pages have them before the server answers next time. A failure
    /// is kept for the library pages' Retry, and the server out of reach shows as such.
    /// </summary>
    public async Task LoadAccount()
    {
        var api = Api.Value;
        if (api == null) return;
        User me;
        List<Library> libraries;
        try
        {
            var meTask = api.Me();
            var librariesTask = api.Libraries();
            me = await meTask;
            libraries = await librariesTask;
        }
        catch (Exception e)
        {
            AccountError.Value = e;
            if (Errors.Unreachable(e)) ServerAway();
            throw;
        }
        AccountError.Value = null;
        Me.Value = me;
        Write(MeKey, me);
        KeepLibraries(libraries);
        _ = KeepProfiles(api);
    }

    async Task KeepProfiles(Api.Api api)
    {
        try
        {
            KeepHousehold(await api.Profiles());
        }
        catch (Exception)
        {
            // Asked again next time.
        }
    }

    void KeepLibraries(IReadOnlyList<Library> libraries)
    {
        Libraries.Value = libraries;
        Write(LibrariesKey, libraries);
    }

    /// <summary>
    /// The profile's own order of its libraries (<paramref name="ids"/>, each once), or the order
    /// everyone sees again (null), on every list of them on all its devices. Changes go one at a
    /// time, in the order they were made; the server's answer is the list from then on, and Home
    /// asks again, since its rows of each library follow the order.
    /// </summary>
    public Task<List<Library>> OrderLibraries(IReadOnlyList<long>? ids)
    {
        async Task<List<Library>> Run(Task before)
        {
            try
            {
                await before;
            }
            catch (Exception)
            {
                // The change before failed: this one goes anyway.
            }
            var api = Api.Value ?? throw new SignedOutException();
            var who = Me.Value?.Id;
            var libraries = ids != null ? await api.SortLibraries(ids) : await api.ForgetLibraryOrder();
            // Another profile, sign-in or server since: the answer was the one before's.
            if (Api.Value == api && Me.Value?.Id == who)
            {
                KeepLibraries(libraries);
                _ = LoadHome();
            }
            return libraries;
        }
        var run = Run(orderQueue);
        orderQueue = run;
        return run;
    }

    public async Task LoadHome()
    {
        var api = Api.Value;
        if (api == null) return;
        try
        {
            var hubs = (await api.Home()).Hubs;
            var failedBefore = HomeError.Value != null;
            Home.Value = hubs;
            HomeError.Value = null;
            Write(HomeKey, hubs);
            if (failedBefore) ServerAnswered();
        }
        catch (Exception e)
        {
            HomeError.Value = e;
            if (Errors.Unreachable(e)) ServerAway();
        }
    }

    // ---- the server out of reach

    /// <summary>
    /// A screen's load got no answer: the notice shows for 6 s (again each time the server goes),
    /// then the rail's mark. Meanwhile the server is asked every 5 s, so the event stream, backing
    /// off up to 30 s between tries, reconnects as soon as it answers and the screens come back.
    /// </summary>
    public void ServerAway()
    {
        if (Offline.Value) return;
        Offline.Value = true;
        OfflineNotice.Value = true;
        noticeTimer?.Dispose();
        noticeTimer = scheduler.After(OfflineNoticeMs, () =>
        {
            noticeTimer = null;
            OfflineNotice.Value = false;
        });
        AskWhileAway();
    }

    void AskWhileAway()
    {
        awayTimer?.Dispose();
        awayTimer = scheduler.After(OfflineAskMs, async () =>
        {
            awayTimer = null;
            var api = Api.Value;
            if (!Offline.Value || api == null) return;
            if (!Visible.Value)
            {
                AskWhileAway();
                return;
            }
            try
            {
                await api.Get<JsonElement>("/api/server", timeoutMs: OfflineAskMs);
            }
            catch (Exception)
            {
                AskWhileAway();
                return;
            }
            if (!Offline.Value) return;
            // A stream waiting to reconnect connects now, and its connection lowers it; one still
            // connected (a page failed in a short blip) can't, so the app does, and asks again.
            if (events?.RetryNow() == true)
            {
                AskWhileAway();
                return;
            }
            ServerAnswered();
            ForgetItems();
            Refresh();
        });
    }

    /// <summary>The server answers again: its events connected, or a screen that had failed got a fresh answer.</summary>
    public void ServerAnswered()
    {
        if (!Offline.Value) return;
        noticeTimer?.Dispose();
        noticeTimer = null;
        awayTimer?.Dispose();
        awayTimer = null;
        Offline.Value = false;
        OfflineNotice.Value = false;
    }

    /// <summary>
    /// Item details the app saw lately, to start a page or a playback without waiting. The
    /// server's events take out what changed; a detail older than 5 minutes goes anyway, in case
    /// one was missed (a resume point moved on another device while the event stream was down).
    /// </summary>
    public ItemDetail? CachedItem(long id)
    {
        if (!items.TryGetValue(id, out var kept)) return null;
        if (scheduler.Now - kept.At <= ItemTtlMs) return kept.Detail;
        items.Remove(id);
        itemOrder.Remove(id);
        return null;
    }

    public async Task<ItemDetail> Item(long id)
    {
        var detail = await Api.Value!.Item(id);
        KeepItem(detail);
        return detail;
    }

    /// <summary>Keeps a detail fetched elsewhere (an item page) for the next page or playback.</summary>
    public void KeepItem(ItemDetail detail)
    {
        itemOrder.Remove(detail.Id);
        itemOrder.AddLast(detail.Id);
        items[detail.Id] = (detail, scheduler.Now);
        if (items.Count > 40)
        {
            items.Remove(itemOrder.First!.Value);
            itemOrder.RemoveFirst();
        }
    }

    void ForgetItems()
    {
        items.Clear();
        itemOrder.Clear();
    }

    /// <summary>
    /// Tracks were picked for <paramref name="detail"/>'s title (the whole show for an episode),
    /// or the profile's defaults changed (null): the details kept name the tracks they started
    /// with before. <paramref name="detail"/>, with the choice in it, is kept; the others go.
    /// </summary>
    public void TracksChanged(ItemDetail? detail)
    {
        ForgetItems();
        if (detail != null) KeepItem(detail);
    }

    /// <summary>
    /// A playback is on top (true) or has ended (false). Meanwhile Home and the pages wait to ask
    /// again; once it ends they do, and the details kept go, since its own reports moved resume
    /// points and watched marks.
    /// </summary>
    public void HoldRefresh(bool on)
    {
        holds = Math.Max(0, holds + (on ? 1 : -1));
        if (on || holds > 0) return;
        ForgetItems();
        _ = LoadHome();
        Changes.Value++;
    }

    /// <summary>Home and the pages that list things ask again; while a playback is on top, once it ends.</summary>
    void Refresh()
    {
        if (holds > 0) return;
        _ = LoadHome();
        // Who is watching and the libraries, when the last try failed or none came yet.
        if (AccountError.Value != null || Libraries.Value == null) _ = LoadAccountQuietly();
        Changes.Value++;
    }

    async Task LoadAccountQuietly()
    {
        try
        {
            await LoadAccount();
        }
        catch (Exception)
        {
            // Kept in AccountError for the library pages' Retry.
        }
    }

    // ---- the server's events

    public IDisposable OnEvent(Action<ServerEvent> listener)
    {
        eventListeners.Add(listener);
        return new Unlisten(() => eventListeners.Remove(listener));
    }

    sealed class Unlisten : IDisposable
    {
        readonly Action undo;

        public Unlisten(Action undo) => this.undo = undo;

        public void Dispose() => undo();
    }

    /// <summary>Follows the server's events while the app is shown and signed in.</summary>
    public void Listen(bool on)
    {
        var server = Server.Value;
        if (!on || server == null || tokens?.SignedIn != true)
        {
            events?.Stop();
            return;
        }
        events ??= new EventStream(server, tokens, HandleEvent, Reconnected, connect, scheduler);
        events.Start();
    }

    void Reconnected()
    {
        // Nothing is replayed: what is older than 30 s, or anything after the server was out of reach, is fetched again.
        var away = Offline.Value;
        ServerAnswered();
        if (away || scheduler.Now - lastSync > 30_000)
        {
            lastSync = scheduler.Now;
            ForgetItems();
            Refresh();
        }
    }

    void HandleEvent(ServerEvent e)
    {
        switch (e.Type)
        {
            case "library_changed":
                // Changes come in bursts during scans: once 1.5 s after the last.
                libraryTimer?.Dispose();
                libraryTimer = scheduler.After(1_500, () =>
                {
                    libraryTimer = null;
                    Refresh();
                });
                break;
            case "libraries_changed":
                _ = LoadAccountQuietly();
                Refresh();
                break;
            case "watchlist_changed":
                Refresh();
                break;
            case "user_state_changed":
                // The item, its parent and grandparent; with `children`, everything under the first as well.
                foreach (var id in e.ItemIds ?? new long[0]) stateIds.Add(id);
                if (e.Children == true && e.ItemIds is { Count: > 0 }) stateParents.Add(e.ItemIds[0]);
                stateTimer ??= scheduler.After(StateGatherMs, ApplyStateChanges);
                break;
        }
        foreach (var l in eventListeners.ToList()) l(e);
    }

    /// <summary>The watch state changes of the last 0.3 s: their details go, and Home and the pages ask again once (with a pending library change).</summary>
    void ApplyStateChanges()
    {
        stateTimer = null;
        bool Under(long? p) => p != null && stateParents.Contains(p.Value);
        foreach (var id in items.Keys.ToList())
        {
            var d = items[id].Detail;
            if (stateIds.Contains(id) || Under(d.ParentId) || Under(d.GrandparentId))
            {
                items.Remove(id);
                itemOrder.Remove(id);
            }
        }
        stateIds.Clear();
        stateParents.Clear();
        libraryTimer?.Dispose();
        libraryTimer = null;
        Refresh();
    }
}
