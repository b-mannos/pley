using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Api;
using Pley.Tv.Core.Auth;
using Pley.Tv.Core.Net;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>Pley's API as one signed-in app.</summary>
public class ApiTests
{
    const string Home = "https://pley.example.com";
    const string Token = Home + "/api/oauth/token";

    static (Api.Api Api, FakeServer Server, FakeClock Clock) SignedIn(string server = Home)
    {
        var clock = new FakeClock();
        var fake = new FakeServer();
        var tokens = new Tokens(server, "client", new MemoryKeyValue(), fake.Transport, null, clock.Read);
        tokens.Install(new StoredTokens(server, "pley_at_0", "pley_rt_0", clock.Now + 3_600_000, "library play", server + "/api/oauth/token", server + "/api/oauth/revoke"), "library play").Wait();
        return (new Api.Api(server, tokens, fake.Transport), fake, clock);
    }

    [Fact]
    public async Task Reads_the_servers_snake_case_and_leaves_missing_fields_null()
    {
        var (api, server, _) = SignedIn();
        server.On("GET", Home + "/api/hubs/home", 200, @"{""hubs"":[{""key"":""continue_watching"",""title"":""Continue Watching"",""library_id"":null,
            ""items"":[{""id"":12,""library_id"":1,""kind"":""episode"",""title"":""Pilot"",""sort_title"":""pilot"",""index"":1,""parent_index"":1,
            ""grandparent_title"":""The Show"",""added_at"":1729600000,""poster_url"":""/api/items/12/images/poster?v=1"",
            ""user_state"":{""position_ms"":61000,""watched"":false,""play_count"":0},""media"":{""bitrate"":8000,""size"":null,""width"":1920,
            ""height"":1080,""video_codec"":""hevc"",""dynamic_range"":""hdr10""},""an_unknown_field"":true}]}]}");
        var hub = Assert.Single((await api.Home()).Hubs);
        Assert.Equal("continue_watching", hub.Key);
        Assert.Null(hub.LibraryId);
        var item = Assert.Single(hub.Items);
        Assert.Equal((12L, "episode", "The Show", 61000L), (item.Id, item.Kind, item.GrandparentTitle, item.UserState.PositionMs));
        Assert.Null(item.Year);
        Assert.Equal("hdr10", item.Media!.DynamicRange);
        var sent = server.Requests.Single();
        Assert.Equal("Bearer pley_at_0", sent.Headers!["Authorization"]);
    }

    [Fact]
    public async Task Refreshes_a_refused_token_once_and_asks_again()
    {
        var (api, server, _) = SignedIn();
        server.On("POST", Token, 200, @"{""access_token"":""pley_at_1"",""token_type"":""Bearer"",""expires_in"":3600,""scope"":""library play"",""refresh_token"":""pley_rt_1""}");
        server.On("GET", Home + "/api/auth/me", req => req.Headers!["Authorization"] == "Bearer pley_at_1"
            ? (200, @"{""id"":1,""username"":""a"",""name"":""A"",""subtitle_mode"":""forced""}")
            : (401, @"{""code"":""unauthorized"",""message"":""expired""}"));
        var me = await api.Me();
        Assert.Equal("forced", me.SubtitleMode);
        Assert.Equal(new[] { "GET", "POST", "GET" }, server.Requests.Select(r => r.Method));
    }

    [Fact]
    public async Task Sends_no_token_to_another_origin()
    {
        var (api, server, _) = SignedIn();
        server.On("GET", "https://images.example.org/poster.jpg", 200, "{}");
        await api.Send("https://images.example.org/poster.jpg");
        Assert.False(server.Requests.Single().Headers!.ContainsKey("Authorization"));
    }

    [Fact]
    public async Task Reads_pleys_errors_and_a_stopped_session()
    {
        var (api, server, _) = SignedIn();
        server.On("GET", Home + "/api/items/9", 404, @"{""code"":""not_found"",""message"":""no such item""}");
        server.On("GET", Home + "/api/items/10", 502, "<html>Bad gateway</html>");
        server.On("GET", Home + "/api/items/11", 410, @"{""code"":""session_stopped"",""message"":""Maintenance tonight""}");
        var missing = await Assert.ThrowsAsync<ApiException>(() => api.Item(9));
        Assert.Equal(("not_found", "No such item"), (missing.Code, Errors.Message(missing)));
        var gateway = await Assert.ThrowsAsync<ApiException>(() => api.Item(10));
        Assert.Equal("internal", gateway.Code);
        Assert.True(Errors.Unreachable(gateway));
        var stopped = await Assert.ThrowsAsync<SessionStoppedException>(() => api.Item(11));
        Assert.Equal("Maintenance tonight", stopped.Note);
    }

    [Fact]
    public async Task Writes_nulls_it_means_and_leaves_out_what_it_keeps()
    {
        var (api, server, _) = SignedIn();
        server.On("PUT", Home + "/api/me/track-defaults", 204);
        server.On("PUT", Home + "/api/items/5/streams", 204);
        server.On("GET", Home + "/api/libraries/3/items?sort=title&watched=false&limit=50", 200, @"{""total"":0,""items"":[]}");
        await api.SetTrackDefaults("en", null, "forced");
        await api.SetStreams(5, null, null, 3, keepAudio: true);
        await api.LibraryItems(3, new LibraryQuery { Sort = LibrarySort.Title, Watched = false, Limit = 50 });
        var bodies = server.Requests.Where(r => r.Body != null).Select(r => r.Body).ToList();
        Assert.Equal(@"{""audio_language"":""en"",""subtitle_language"":null,""subtitle_mode"":""forced""}", bodies[0]);
        Assert.Equal(@"{""subtitle_index"":3}", bodies[1]);
    }

    [Fact]
    public async Task At_home_gives_up_after_3_s_when_the_server_has_gone_but_waits_for_a_slow_one()
    {
        var gone = new FakeServer();
        var tokens = new Tokens("http://nas:7539", "client", new MemoryKeyValue(), gone.Transport);
        await tokens.Install(new StoredTokens("http://nas:7539", "pley_at_0", "pley_rt_0", long.MaxValue / 2, "library", "http://nas:7539/t", "http://nas:7539/r"), "library");
        var hanging = new TaskCompletionSource<HttpResponse>();
        Transport slow = req => req.Url.EndsWith("/api/server")
            ? Task.FromException<HttpResponse>(new NetworkException("Can't reach the server"))
            : hanging.Task;
        var api = new Api.Api("http://nas:7539", tokens, slow);
        var e = await Assert.ThrowsAsync<NetworkException>(() => api.Libraries());
        Assert.Equal("No answer in time.", Errors.UnreachableReason(e));

        var answering = new TaskCompletionSource<HttpResponse>();
        Transport busy = req => req.Url.EndsWith("/api/server")
            ? Task.FromResult(new HttpResponse(200, _ => null, @"{""version"":""1""}"))
            : answering.Task;
        var patient = new Api.Api("http://nas:7539", tokens, busy);
        var asked = patient.Libraries();
        await Task.Delay(Api.Api.HomeWaitMs + 500);
        Assert.False(asked.IsCompleted);
        answering.SetResult(new HttpResponse(200, _ => null, "[]"));
        Assert.Empty(await asked);
    }
}
