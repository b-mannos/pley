using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Api;
using Pley.Tv.Core.Auth;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>What the TV keeps, and how it follows the server.</summary>
public class StateTests
{
    const string Home = "https://pley.example.com";

    static AppState State(MemoryKeyValue store, FakeServer server, FakeScheduler scheduler, EventConnect? connect = null) =>
        new AppState(new AppPlatform("tizen", "client", store), server.Transport, connect ?? ((_, _, _, _, _) => Task.FromResult(0)), scheduler);

    static void SignIn(AppState state, MemoryKeyValue store, FakeScheduler scheduler)
    {
        state.Use(Home);
        state.TokenStore!.Install(new StoredTokens(Home, "pley_at_0", "pley_rt_0", scheduler.Now + 3_600_000, "library play", Home + "/t", Home + "/r"), "library play").Wait();
    }

    [Fact]
    public void Starts_on_the_first_screen_the_TV_kept_for()
    {
        var store = new MemoryKeyValue();
        var scheduler = new FakeScheduler();
        var state = State(store, new FakeServer(), scheduler);
        Assert.Equal(StartPage.Servers, state.Start());
        state.Use(Home + "/");
        Assert.Equal(Home, state.Server.Value);
        Assert.Equal(StartPage.Link, state.Start());
        SignIn(state, store, scheduler);
        Assert.Equal(StartPage.Home, state.Start());
        state.KeepHousehold(new[] { new HouseholdProfile { Id = 1, Name = "A" }, new HouseholdProfile { Id = 2, Name = "B" } });
        Assert.Equal(StartPage.Profiles, state.Start());
        state.SetAlwaysStartAs(2);
        Assert.Equal(StartPage.Home, state.Start());
        // A new launch reads the same from the store.
        Assert.Equal(StartPage.Home, State(store, new FakeServer(), scheduler).Start());
    }

    [Fact]
    public void Lists_servers_most_recently_used_first_and_keeps_an_install_id()
    {
        var store = new MemoryKeyValue();
        var scheduler = new FakeScheduler();
        var state = State(store, new FakeServer(), scheduler);
        state.RememberServer("http://nas:7539", "NAS");
        scheduler.Advance(1000);
        state.RememberServer(Home + "/", "Home");
        scheduler.Advance(1000);
        state.RememberServer("http://nas:7539/", "NAS again");
        Assert.Equal(new[] { ("http://nas:7539", "NAS again"), (Home, "Home") }, state.SavedServers().Select(s => (s.Url, s.Name)));
        var id = state.InstallId();
        Assert.Matches("^tizen-[0-9a-f]{32}$", id);
        Assert.Equal(id, State(store, new FakeServer(), scheduler).InstallId());
    }

    [Fact]
    public void Leaves_out_what_was_saved_in_another_shape()
    {
        var store = new MemoryKeyValue();
        store.Set("pley.home", "{\"not\":\"a list\"}");
        store.Set("pley.me", "[1,2]");
        store.Set("pley.settings", "{\"quality\":\"original\",\"autoplayNext\":false}");
        var state = State(store, new FakeServer(), new FakeScheduler());
        Assert.Null(state.Home.Value);
        Assert.Null(state.Me.Value);
        // Original saved before Automatic, never picked: Automatic now. The rest is kept.
        Assert.Equal(("auto", false), (state.Settings.Value.Quality, state.Settings.Value.AutoplayNext));
    }

    [Fact]
    public async Task Shows_the_notice_for_6_s_when_the_server_goes_and_comes_back_when_it_answers()
    {
        var store = new MemoryKeyValue();
        var scheduler = new FakeScheduler();
        var server = new FakeServer();
        var state = State(store, server, scheduler);
        SignIn(state, store, scheduler);
        await state.LoadHome();
        Assert.True(state.Offline.Value);
        Assert.True(state.OfflineNotice.Value);
        Assert.Equal("Nothing answers there.", Errors.UnreachableReason(state.HomeError.Value!));
        // The first question, after 5 s, gets no answer either; the notice goes after 6 s.
        scheduler.Advance(5_000);
        await Task.Delay(50);
        scheduler.Advance(1_000);
        Assert.False(state.OfflineNotice.Value);
        Assert.True(state.Offline.Value);
        server.On("GET", Home + "/api/server", 200, @"{""version"":""1""}");
        server.On("GET", Home + "/api/hubs/home", 200, @"{""hubs"":[]}");
        // Coming back asks for who is watching too, none having come yet.
        server.On("GET", Home + "/api/auth/me", 200, @"{""id"":1,""username"":""a"",""name"":""A"",""subtitle_mode"":""off""}");
        server.On("GET", Home + "/api/libraries", 200, "[]");
        server.On("GET", Home + "/api/oauth/profiles", 200, "[]");
        scheduler.Advance(5_000);
        await Task.Delay(50);
        Assert.False(state.Offline.Value);
        Assert.Empty(state.Home.Value!);
    }

    [Fact]
    public async Task Gathers_watch_state_changes_and_asks_again_once()
    {
        var store = new MemoryKeyValue();
        var scheduler = new FakeScheduler();
        var server = new FakeServer();
        var homes = 0;
        server.On("GET", Home + "/api/hubs/home", _ =>
        {
            homes++;
            return (200, @"{""hubs"":[]}");
        });
        Action<string>? push = null;
        Action? open = null;
        var state = State(store, server, scheduler, (url, token, onOpen, onText, cancel) =>
        {
            push = onText;
            open = onOpen;
            return new TaskCompletionSource<int>().Task;
        });
        SignIn(state, store, scheduler);
        var heard = new List<string>();
        state.OnEvent(e => heard.Add(e.Type));
        state.Listen(true);
        await Task.Delay(50);
        // Connecting forgets the details kept, since nothing is replayed: these come after.
        open!();
        await Task.Delay(50);
        state.KeepItem(new ItemDetail { Id = 5, ParentId = 7, GrandparentId = 3 });
        state.KeepItem(new ItemDetail { Id = 9 });
        var homesAfterConnecting = homes;
        push!("data: {\"type\":\"user_state_changed\",\"item_ids\":[7,3],\"children\":true}\n\n");
        push!("data: {\"type\":\"user_state_changed\",\"item_ids\":[8]}\n\ndata: {\"type\":\"sessions_");
        push!("changed\"}\n\n");
        Assert.Equal(new[] { "user_state_changed", "user_state_changed", "sessions_changed" }, heard);
        scheduler.Advance(300);
        await Task.Delay(50);
        // Everything under item 7, the first named with `children`, goes; item 9 stays; Home is asked again once.
        Assert.Null(state.CachedItem(5));
        Assert.NotNull(state.CachedItem(9));
        Assert.Equal(homesAfterConnecting + 1, homes);
    }

    [Fact]
    public void Reads_server_sent_events_split_anywhere()
    {
        var reader = new SseReader();
        Assert.Empty(reader.Feed("event: x\r\ndata: {\"type\":\"library_"));
        var events = reader.Feed("changed\",\"library_id\":4}\r\n\r\n: comment\n\ndata: not json\n\ndata: {\"type\":\"watchlist_changed\"}\n\n");
        Assert.Equal(new[] { ("library_changed", (long?)4), ("watchlist_changed", null) }, events.Select(e => (e.Type, e.LibraryId)));
    }

    [Fact]
    public async Task Reconnects_with_growing_waits_and_at_once_after_a_long_stream()
    {
        var scheduler = new FakeScheduler();
        var store = new MemoryKeyValue();
        var tokens = new Tokens(Home, "client", store, new FakeServer().Transport, null, () => scheduler.Now);
        await tokens.Install(new StoredTokens(Home, "a", "r", long.MaxValue / 2, "library", Home + "/t", Home + "/r"), "library");
        var connects = new List<long>();
        var stream = new EventStream(Home, tokens, _ => { }, () => { }, (url, token, onOpen, onText, cancel) =>
        {
            connects.Add(scheduler.Now);
            return Task.FromResult(0);
        }, scheduler);
        stream.Start();
        await Task.Delay(20);
        foreach (var _ in Enumerable.Range(0, 6))
        {
            scheduler.Advance(30_000);
            await Task.Delay(20);
        }
        var waits = connects.Zip(connects.Skip(1), (a, b) => b - a).ToList();
        Assert.Equal(new long[] { 2_000, 4_000, 8_000, 16_000, 30_000 }, waits.Take(5));
        stream.Stop();
    }
}
