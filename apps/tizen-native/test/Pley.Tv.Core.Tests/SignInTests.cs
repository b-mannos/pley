using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Auth;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>Signing a TV in with a code, and keeping its tokens.</summary>
public class SignInTests
{
    const string Home = "https://pley.example.com";
    const string Token = Home + "/api/oauth/token";
    const string Revoke = Home + "/api/oauth/revoke";
    const string Device = Home + "/api/oauth/device";
    const string ClientId = "https://pley-media-server.github.io/pley/oauth/tizen.json";

    static readonly string MetadataJson = $@"{{""issuer"":""{Home}"",""token_endpoint"":""{Token}"",""revocation_endpoint"":""{Revoke}"",
        ""device_authorization_endpoint"":""{Device}"",""scopes_supported"":[""library"",""play"",""playlists"",""profiles"",""admin""],
        ""grant_types_supported"":[""refresh_token"",""urn:ietf:params:oauth:grant-type:device_code""]}}";

    static string Tokens(string access, string refresh, string scope = "library play playlists profiles") =>
        $@"{{""access_token"":""{access}"",""token_type"":""Bearer"",""expires_in"":3600,""scope"":""{scope}"",""refresh_token"":""{refresh}""}}";

    static DeviceCode Code(FakeClock clock, int interval = 5) =>
        new DeviceCode(Home, Home, "dc", "BCDF-GHJK", Home + "/link", null, clock.Now + 600_000, interval, Token, Revoke, "library play");

    [Fact]
    public async Task Start_asks_for_pleys_own_scopes_that_the_server_offers()
    {
        var server = new FakeServer()
            .On("GET", Home + Metadata.Path, 200, MetadataJson)
            .On("POST", Device, 200, $@"{{""device_code"":""dc"",""user_code"":""BCDF-GHJK"",""verification_uri"":""{Home}/link"",""expires_in"":600,""interval"":5}}");
        var code = await new DeviceSignIn(server.Transport, ClientId, new FakeClock().Read).Start(Home + "/");
        Assert.Equal("BCDF-GHJK", code.UserCode);
        Assert.Equal("library play playlists profiles", code.Scope);
        var sent = server.Requests.Last();
        Assert.Contains("scope=library%20play%20playlists%20profiles", sent.Body);
        Assert.Contains("client_id=" + System.Uri.EscapeDataString(ClientId), sent.Body);
    }

    [Fact]
    public async Task Start_refuses_plain_http_beyond_the_home_network()
    {
        var e = await Assert.ThrowsAsync<AuthException>(() => new DeviceSignIn(new FakeServer().Transport, ClientId).Start("http://pley.example.com"));
        Assert.Equal(Addresses.Unencrypted, e.Message);
    }

    [Fact]
    public async Task Start_says_a_server_from_before_the_TV_apps_needs_updating()
    {
        var server = new FakeServer()
            .On("GET", Home + Metadata.Path, 200, MetadataJson)
            .On("POST", Device, 401, @"{""error"":""invalid_client""}");
        var e = await Assert.ThrowsAsync<AuthException>(() => new DeviceSignIn(server.Transport, ClientId).Start(Home));
        Assert.Equal("this server doesn't know Pley's TV apps yet: update it", e.Message);
    }

    [Fact]
    public async Task Poll_waits_slows_down_when_told_and_signs_in()
    {
        var clock = new FakeClock();
        var answers = new Queue<(int, string?)>(new (int, string?)[]
        {
            (400, @"{""error"":""authorization_pending""}"),
            (400, @"{""error"":""slow_down""}"),
            (200, Tokens("pley_at_1", "pley_rt_1", "library play")),
        });
        var server = new FakeServer().On("POST", Token, _ => answers.Dequeue());
        var updates = new List<DevicePoll>();
        var result = await new DeviceSignIn(server.Transport, ClientId, clock.Read, clock.Sleep).Poll(Code(clock), updates.Add, CancellationToken.None);
        var signedIn = Assert.IsType<DevicePoll.SignedIn>(result);
        Assert.Equal("pley_at_1", signedIn.Tokens.AccessToken);
        // Every wait 5 s, and 5 s more after slow_down, for good.
        Assert.Equal(new long[] { 5000, 5000, 10000 }, clock.Sleeps);
        Assert.IsType<DevicePoll.Waiting>(updates.First());
        Assert.IsType<DevicePoll.SignedIn>(updates.Last());
    }

    [Fact]
    public async Task Poll_backs_off_while_the_server_is_unreachable_and_says_so()
    {
        var clock = new FakeClock();
        var answers = new Queue<(int, string?)>(new (int, string?)[]
        {
            (0, null),
            (0, null),
            (0, null),
            (400, @"{""error"":""authorization_pending""}"),
            (400, @"{""error"":""access_denied""}"),
        });
        var server = new FakeServer().On("POST", Token, _ => answers.Dequeue());
        var updates = new List<DevicePoll>();
        var result = await new DeviceSignIn(server.Transport, ClientId, clock.Read, clock.Sleep).Poll(Code(clock, interval: 1), updates.Add, CancellationToken.None);
        Assert.IsType<DevicePoll.Denied>(result);
        // 1 s, then 2, 4 and 8 s after one, two and three failures, then back to 1 s.
        Assert.Equal(new long[] { 1000, 2000, 4000, 8000, 1000 }, clock.Sleeps);
        Assert.Equal(new long[] { 2000, 4000, 8000 }, updates.OfType<DevicePoll.Unreachable>().Select(u => u.RetryInMs));
        // Back to waiting once the server answers again.
        Assert.IsType<DevicePoll.Waiting>(updates[updates.Count - 2]);
    }

    [Fact]
    public async Task Poll_ends_when_the_code_expires_and_when_stopped()
    {
        var clock = new FakeClock();
        var server = new FakeServer().On("POST", Token, 400, @"{""error"":""authorization_pending""}");
        var code = Code(clock) with { ExpiresAt = clock.Now + 12_000 };
        var result = await new DeviceSignIn(server.Transport, ClientId, clock.Read, clock.Sleep).Poll(code, _ => { }, CancellationToken.None);
        Assert.IsType<DevicePoll.Expired>(result);
        Assert.Equal(new long[] { 5000, 5000, 2000 }, clock.Sleeps);

        using var stop = new CancellationTokenSource();
        stop.Cancel();
        var stopped = await new DeviceSignIn(server.Transport, ClientId, clock.Read, clock.Sleep).Poll(Code(clock), _ => { }, stop.Token);
        Assert.IsType<DevicePoll.Waiting>(stopped);
    }

    static (Tokens Tokens, FakeServer Server, MemoryKeyValue Store, FakeClock Clock, List<SignOutReason> SignedOut) Kept(StoredTokens? saved = null)
    {
        var clock = new FakeClock();
        var server = new FakeServer().On("POST", Revoke, 200, "{}");
        var store = new MemoryKeyValue();
        var signedOut = new List<SignOutReason>();
        var tokens = new Tokens(Home, ClientId, store, server.Transport, signedOut.Add, clock.Read);
        saved ??= new StoredTokens(Home, "pley_at_0", "pley_rt_0", clock.Now + 3_600_000, "library play", Token, Revoke);
        tokens.Install(saved, "library play").Wait();
        return (tokens, server, store, clock, signedOut);
    }

    [Fact]
    public async Task An_access_token_is_used_until_a_minute_before_it_ends_then_refreshed_once()
    {
        var (tokens, server, _, clock, _) = Kept();
        var refreshes = 0;
        server.On("POST", Token, _ =>
        {
            refreshes++;
            return (200, Tokens($"pley_at_{refreshes}", $"pley_rt_{refreshes}", "library play"));
        });
        Assert.Equal("pley_at_0", await tokens.AccessToken());
        clock.Now += 3_600_000 - 30_000;
        // Two callers at once: one refresh, and both get its token.
        var both = await Task.WhenAll(tokens.AccessToken(), tokens.AccessToken());
        Assert.Equal(new[] { "pley_at_1", "pley_at_1" }, both);
        Assert.Equal(1, refreshes);
        Assert.Equal("pley_rt_1", tokens.Load()!.RefreshToken);
    }

    [Fact]
    public async Task A_spent_refresh_token_signs_the_TV_out()
    {
        var (tokens, server, store, _, signedOut) = Kept();
        server.On("POST", Token, 400, @"{""error"":""invalid_grant""}");
        await Assert.ThrowsAsync<SignedOutException>(() => tokens.Refresh("pley_at_0"));
        Assert.Equal(new[] { SignOutReason.Ended }, signedOut);
        Assert.Empty(store.Values);
    }

    [Fact]
    public async Task A_refresh_that_widens_the_scope_ends_the_grant()
    {
        var (tokens, server, store, _, signedOut) = Kept();
        server.On("POST", Token, 200, Tokens("pley_at_1", "pley_rt_1", "library play admin"));
        var e = await Assert.ThrowsAsync<AuthException>(() => tokens.Refresh("pley_at_0"));
        Assert.Equal(TokenRequests.WiderScopeMessage, e.Message);
        Assert.Equal(new[] { SignOutReason.WiderScope }, signedOut);
        Assert.Empty(store.Values);
        Assert.Contains(server.Requests, r => r.Url == Revoke && r.Body!.Contains("token=pley_rt_1"));
    }

    [Fact]
    public async Task A_new_sign_in_ends_the_one_it_replaces()
    {
        var (tokens, server, _, clock, _) = Kept();
        await tokens.Install(new StoredTokens(Home, "pley_at_9", "pley_rt_9", clock.Now + 3_600_000, "library", Token, Revoke), "library play");
        Assert.Contains(server.Requests, r => r.Url == Revoke && r.Body!.Contains("token=pley_rt_0"));
        Assert.Equal("pley_at_9", tokens.Load()!.AccessToken);
    }

    [Fact]
    public async Task A_token_refused_right_after_its_refresh_ends_the_sign_in()
    {
        var (tokens, server, _, clock, signedOut) = Kept();
        server.On("POST", Token, 200, Tokens("pley_at_1", "pley_rt_1", "library play"));
        Assert.False(await tokens.RefusedAfterRefresh("pley_at_0"));
        var fresh = await tokens.Refresh("pley_at_0");
        clock.Now += 10_000;
        Assert.True(await tokens.RefusedAfterRefresh(fresh));
        Assert.Equal(new[] { SignOutReason.Ended }, signedOut);
    }

    [Fact]
    public async Task A_wrong_PIN_changes_nothing()
    {
        var (tokens, server, _, _, signedOut) = Kept();
        server.On("POST", Token, 400, @"{""error"":""invalid_grant"",""error_description"":""That PIN isn't right"",""reason"":""wrong_pin""}");
        var e = await Assert.ThrowsAsync<TokenException>(() => tokens.SwitchProfile(2, "0000"));
        Assert.Equal("That PIN isn't right", e.Message);
        Assert.Empty(signedOut);
        Assert.Equal("pley_at_0", tokens.Load()!.AccessToken);
    }
}
