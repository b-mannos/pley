using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Auth;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Api;

/// <summary>An answer that isn't 2xx, with Pley's <c>{ code, message }</c>.</summary>
public class ApiException : Exception
{
    public ApiException(int status, string code, string message, JsonElement? body = null) : base(message)
    {
        Status = status;
        Code = code;
        Body = body;
    }

    public int Status { get; }
    public string Code { get; }
    /// <summary>The answer's JSON, when it had some (fields past <c>code</c> and <c>message</c>).</summary>
    public JsonElement? Body { get; }
}

/// <summary>A playback session stopped from elsewhere (<c>410 session_stopped</c>), with what the one who stopped it wrote.</summary>
public class SessionStoppedException : ApiException
{
    public SessionStoppedException(string? note) : base(410, "session_stopped", note ?? "Playback was stopped by the server's administrator") => Note = note;

    public string? Note { get; }
}

/// <summary>How errors read to people, and whether the server answered at all.</summary>
public static class Errors
{
    /// <summary>What to show for an error, as a sentence.</summary>
    public static string Message(Exception e)
    {
        var text = e.Message;
        return text.Length > 0 ? char.ToUpperInvariant(text[0]) + text.Substring(1) : text;
    }

    /// <summary>The server answered nothing: no connection, no answer in time, or a gateway in front of it saying it has none (502–504).</summary>
    public static bool Unreachable(Exception e) => e switch
    {
        NetworkException n => !n.Cancelled,
        ApiException a => a.Status is 502 or 503 or 504,
        _ => false,
    };

    /// <summary>Why the server answered nothing, as the Android app says it: "Nothing answers there." or "No answer in time."</summary>
    public static string UnreachableReason(Exception e) =>
        e is NetworkException n && n.Message == Transports.TimedOut ? "No answer in time." : "Nothing answers there.";
}

// Answers whose shape is an envelope around the types.
public sealed record HubsAnswer(IReadOnlyList<Hub> Hubs);
public sealed record ItemsPage(long Total, IReadOnlyList<ItemSummary> Items);
public sealed record ItemsAnswer(IReadOnlyList<ItemSummary> Items);
public sealed record ItemAnswer(ItemSummary? Item);
public sealed record FiltersAnswer(IReadOnlyList<string> Genres, IReadOnlyList<int> Years);
public sealed record CollectionsAnswer(IReadOnlyList<Collection> Collections);
public sealed record CollectionDetail : Collection
{
    public IReadOnlyList<ItemSummary> Items { get; init; } = new ItemSummary[0];
}
public sealed record PlaylistsAnswer(IReadOnlyList<Playlist> Playlists);
public sealed record PlaylistDetail : Playlist
{
    public IReadOnlyList<PlaylistEntry> Entries { get; init; } = new PlaylistEntry[0];
}
public sealed record SearchAnswer(IReadOnlyList<ItemSummary> Items, IReadOnlyList<PersonSummary>? People);
public sealed record CollectionRef(long Id, string Title);
public sealed record ItemCollectionsAnswer(IReadOnlyList<CollectionRef> Collections);
public sealed record AddedAnswer(long Added);
public sealed record HistoryPage(long Total, IReadOnlyList<HistoryEntry> Entries);

/// <summary>Query values of <see cref="Api.LibraryItems"/>; null ones are left out.</summary>
public sealed record LibraryQuery
{
    public string? Sort { get; init; }
    /// <summary>"asc" or "desc".</summary>
    public string? Order { get; init; }
    public bool? Watched { get; init; }
    public string? Genre { get; init; }
    public int? Year { get; init; }
    public int? Offset { get; init; }
    public int? Limit { get; init; }
}

/// <summary>
/// Pley's API for one server, as one signed-in app: every request carries the access token in
/// <c>Authorization: Bearer</c> (the only place Pley takes an app's token), refreshed once on
/// <c>invalid_token</c>. The web TV app's <c>api/client.ts</c>.
/// </summary>
public sealed class Api
{
    /// <summary>A server at home accepts at once or never: after this long without an answer, a quick question tells which.</summary>
    public const int HomeWaitMs = 3_000;

    static readonly Regex Absolute = new Regex(@"^https?://", RegexOptions.CultureInvariant);
    static readonly Regex InvalidToken = new Regex("invalid_token", RegexOptions.CultureInvariant);

    /// <summary>Request bodies: written as given, nulls included (a null clears a field; a field left out keeps it).</summary>
    static readonly JsonSerializerOptions BodyOptions = new JsonSerializerOptions();

    readonly Tokens tokens;
    readonly Transport http;
    /// <summary>The server is on the home network: a request it doesn't answer within 3 s gets a quick question.</summary>
    readonly bool homeNetwork;

    public Api(string server, Tokens tokens, Transport http)
    {
        Server = server;
        this.tokens = tokens;
        this.http = http;
        homeNetwork = Addresses.AtHome(server);
    }

    public string Server { get; }

    /// <summary>An absolute address on this server for <paramref name="path"/> ("/api/…").</summary>
    public string Url(string path) => Absolute.IsMatch(path) ? path : Server + path;

    /// <summary>
    /// Whether <paramref name="path"/> is on this server: an address the server hands out (a
    /// picture's, a playlist's) that names another origin gets no token, as Android's (SECURITY.md).
    /// </summary>
    bool OwnServer(string path) => Addresses.Origin(Url(path)) == Addresses.Origin(Server);

    /// <summary>The raw answer of an authenticated request; errors as <see cref="ApiException"/>.</summary>
    public async Task<HttpResponse> Send(string path, string method = "GET", object? body = null, IEnumerable<KeyValuePair<string, object?>>? query = null,
        CancellationToken cancel = default, int? timeoutMs = null, ResponseType responseType = ResponseType.Json, Action<long>? onProgress = null)
    {
        var token = await tokens.AccessToken().ConfigureAwait(false) ?? throw new SignedOutException();
        var res = await Once(path, method, body, query, cancel, timeoutMs, responseType, onProgress, token).ConfigureAwait(false);
        if (res.Status == 401 && IsInvalidToken(res) && OwnServer(path))
        {
            // Refused just after a refresh: the server no longer takes this sign-in.
            if (await tokens.RefusedAfterRefresh(token).ConfigureAwait(false)) throw new SignedOutException();
            var fresh = await tokens.Refresh(token).ConfigureAwait(false);
            res = await Once(path, method, body, query, cancel, timeoutMs, responseType, onProgress, fresh).ConfigureAwait(false);
        }
        if (res.Status >= 200 && res.Status < 300) return res;
        throw ToError(res);
    }

    Task<HttpResponse> Once(string path, string method, object? body, IEnumerable<KeyValuePair<string, object?>>? query, CancellationToken cancel,
        int? timeoutMs, ResponseType responseType, Action<long>? onProgress, string token)
    {
        var headers = new Dictionary<string, string> { ["Accept"] = "application/json" };
        if (OwnServer(path)) headers["Authorization"] = "Bearer " + token;
        string? text = null;
        if (body != null)
        {
            headers["Content-Type"] = "application/json";
            text = JsonSerializer.Serialize(body, BodyOptions);
        }
        var req = new HttpRequest
        {
            Method = method,
            Url = Transports.WithQuery(Url(path), query),
            Headers = headers,
            Body = text,
            ResponseType = responseType,
            TimeoutMs = timeoutMs ?? Transports.DefaultTimeoutMs,
            Cancel = cancel,
            OnProgress = onProgress,
        };
        return homeNetwork ? Watched(req, Url("/api/server")) : http(req);
    }

    /// <summary>
    /// <paramref name="req"/> to a server on the home network, which a TV can't give a connect
    /// timeout of its own (the Android app gives it 3 s): when it hasn't answered in 3 s, the
    /// server is asked a quick question, and if that gets no answer in 3 s either, the request
    /// gives up with "no answer in time". A server that is only slow (a conversion starting)
    /// answers the question, and the request waits as long as it was going to.
    /// </summary>
    async Task<HttpResponse> Watched(HttpRequest req, string ping)
    {
        using var own = CancellationTokenSource.CreateLinkedTokenSource(req.Cancel);
        var main = http(req with { Cancel = own.Token });
        // A request given up on still fails later; that failure is seen here, not left unobserved.
        _ = main.ContinueWith(t => t.Exception, TaskContinuationOptions.OnlyOnFaulted);
        var waited = await Task.WhenAny(main, Task.Delay(HomeWaitMs, own.Token)).ConfigureAwait(false);
        if (waited == main || own.IsCancellationRequested) return await main.ConfigureAwait(false);
        var pingFailed = http(new HttpRequest
        {
            Url = ping,
            Headers = new Dictionary<string, string> { ["Accept"] = "application/json" },
            ResponseType = ResponseType.Json,
            TimeoutMs = HomeWaitMs,
            Cancel = own.Token,
        }).ContinueWith(t => t.Exception?.InnerException is NetworkException { Cancelled: false }, TaskScheduler.Default);
        if (await Task.WhenAny(main, pingFailed).ConfigureAwait(false) == main || !await pingFailed.ConfigureAwait(false))
            return await main.ConfigureAwait(false);
        own.Cancel();
        throw new NetworkException(Transports.TimedOut);
    }

    public async Task<T> Get<T>(string path, IEnumerable<KeyValuePair<string, object?>>? query = null, CancellationToken cancel = default, int? timeoutMs = null)
    {
        var res = await Send(path, query: query, cancel: cancel, timeoutMs: timeoutMs).ConfigureAwait(false);
        return Read<T>(res);
    }

    public async Task<JsonElement?> Call(string method, string path, object? body = null, CancellationToken cancel = default)
    {
        var res = await Send(path, method, body, cancel: cancel).ConfigureAwait(false);
        return res.Json();
    }

    async Task<T> Call<T>(string method, string path, object? body = null, CancellationToken cancel = default) =>
        Read<T>(await Send(path, method, body, cancel: cancel).ConfigureAwait(false));

    static T Read<T>(HttpResponse res)
    {
        try
        {
            return JsonSerializer.Deserialize<T>(res.Text ?? "null", ApiJson.Options) ?? throw new ApiException(res.Status, "bad_answer", "the server's answer can't be read");
        }
        catch (JsonException)
        {
            throw new ApiException(res.Status, "bad_answer", "the server's answer can't be read");
        }
    }

    static KeyValuePair<string, object?> Q(string key, object? value) => new KeyValuePair<string, object?>(key, value);

    // ---- account

    public Task<User> Me(CancellationToken cancel = default) => Get<User>("/api/auth/me", cancel: cancel);

    public Task<List<HouseholdProfile>> Profiles(CancellationToken cancel = default) => Get<List<HouseholdProfile>>("/api/oauth/profiles", cancel: cancel);

    public Task<JsonElement?> SetTrackDefaults(string? audio, string? subtitle, string mode) =>
        Call("PUT", "/api/me/track-defaults", new Dictionary<string, object?> { ["audio_language"] = audio, ["subtitle_language"] = subtitle, ["subtitle_mode"] = mode });

    // ---- browsing

    public Task<HubsAnswer> Home(CancellationToken cancel = default) => Get<HubsAnswer>("/api/hubs/home", cancel: cancel);

    public Task<List<Library>> Libraries(CancellationToken cancel = default) => Get<List<Library>>("/api/libraries", cancel: cancel);

    /// <summary>The profile's own order of its libraries, <paramref name="ids"/> naming each once (scope play); them in it.</summary>
    public Task<List<Library>> SortLibraries(IReadOnlyList<long> ids) => Call<List<Library>>("PUT", "/api/me/libraries/order", new Dictionary<string, object?> { ["ids"] = ids });

    /// <summary>The order everyone sees, for this profile again (scope play); the libraries in it.</summary>
    public Task<List<Library>> ForgetLibraryOrder() => Call<List<Library>>("DELETE", "/api/me/libraries/order");

    public Task<ItemsPage> LibraryItems(long id, LibraryQuery query, CancellationToken cancel = default) =>
        Get<ItemsPage>($"/api/libraries/{id}/items", new[]
        {
            Q("sort", query.Sort), Q("order", query.Order), Q("watched", query.Watched), Q("genre", query.Genre),
            Q("year", query.Year), Q("offset", query.Offset), Q("limit", query.Limit),
        }, cancel);

    public Task<HubsAnswer> LibraryHubs(long id, CancellationToken cancel = default) => Get<HubsAnswer>($"/api/libraries/{id}/hubs", cancel: cancel);

    public Task<FiltersAnswer> LibraryFilters(long id, CancellationToken cancel = default) => Get<FiltersAnswer>($"/api/libraries/{id}/filters", cancel: cancel);

    public Task<CollectionsAnswer> Collections(long libraryId, CancellationToken cancel = default) => Get<CollectionsAnswer>($"/api/libraries/{libraryId}/collections", cancel: cancel);

    public Task<CollectionDetail> Collection(long id, CancellationToken cancel = default) => Get<CollectionDetail>($"/api/collections/{id}", cancel: cancel);

    public Task<ItemDetail> Item(long id, CancellationToken cancel = default) => Get<ItemDetail>($"/api/items/{id}", cancel: cancel);

    public Task<ItemsAnswer> Children(long id, CancellationToken cancel = default) => Get<ItemsAnswer>($"/api/items/{id}/children", cancel: cancel);

    public Task<ItemAnswer> OnDeck(long id, CancellationToken cancel = default) => Get<ItemAnswer>($"/api/items/{id}/on-deck", cancel: cancel);

    public Task<ItemAnswer> Next(long id, CancellationToken cancel = default) => Get<ItemAnswer>($"/api/items/{id}/next", cancel: cancel);

    public Task<ItemsAnswer> Related(long id, CancellationToken cancel = default) => Get<ItemsAnswer>($"/api/items/{id}/related", new[] { Q("limit", 20) }, cancel);

    public Task<PersonDetail> Person(long id, CancellationToken cancel = default) => Get<PersonDetail>($"/api/people/{id}", cancel: cancel);

    /// <summary>Items and people; Live TV channels come with Live TV.</summary>
    public Task<SearchAnswer> Search(string q, CancellationToken cancel = default) => Get<SearchAnswer>("/api/search", new[] { Q("q", q), Q("limit", 60) }, cancel);

    public Task<PlaylistsAnswer> Playlists(CancellationToken cancel = default) => Get<PlaylistsAnswer>("/api/playlists", cancel: cancel);

    public Task<PlaylistDetail> Playlist(long id, CancellationToken cancel = default) => Get<PlaylistDetail>($"/api/playlists/{id}", cancel: cancel);

    /// <summary>The collections an item is part of.</summary>
    public Task<ItemCollectionsAnswer> ItemCollections(long id, CancellationToken cancel = default) => Get<ItemCollectionsAnswer>($"/api/items/{id}/collections", cancel: cancel);

    public Task<AddedAnswer> AddToPlaylist(long playlistId, IReadOnlyList<long> itemIds) =>
        Call<AddedAnswer>("POST", $"/api/playlists/{playlistId}/items", new Dictionary<string, object?> { ["item_ids"] = itemIds });

    /// <param name="kind">"video", "audio", or null for the server to tell from the items.</param>
    public Task<Playlist> CreatePlaylist(string title, string? kind, IReadOnlyList<long> itemIds)
    {
        var body = new Dictionary<string, object?> { ["title"] = title };
        if (kind != null) body["kind"] = kind;
        body["item_ids"] = itemIds;
        return Call<Playlist>("POST", "/api/playlists", body);
    }

    public Task<JsonElement?> RenamePlaylist(long id, string title) => Call("PATCH", $"/api/playlists/{id}", new Dictionary<string, object?> { ["title"] = title });

    public Task<JsonElement?> DeletePlaylist(long id) => Call("DELETE", $"/api/playlists/{id}");

    /// <summary>Takes one entry out (the item stays in its library).</summary>
    public Task<JsonElement?> RemoveFromPlaylist(long id, long entryId) => Call("DELETE", $"/api/playlists/{id}/items/{entryId}");

    /// <summary>The entries in their new order, every one of them.</summary>
    public Task<JsonElement?> ReorderPlaylist(long id, IReadOnlyList<long> entryIds) =>
        Call("PUT", $"/api/playlists/{id}/order", new Dictionary<string, object?> { ["entry_ids"] = entryIds });

    /// <summary>A smart playlist: a library's listing by <paramref name="filter"/> (Plex's syntax), which keeps up with the library by itself.</summary>
    public Task<Playlist> CreateSmartPlaylist(string title, long libraryId, string filter) =>
        Call<Playlist>("POST", "/api/playlists", new Dictionary<string, object?> { ["title"] = title, ["smart_filter"] = filter, ["library_id"] = libraryId });

    // ---- watching

    public Task<HistoryPage> History(int offset, int limit, CancellationToken cancel = default) =>
        Get<HistoryPage>("/api/history", new[] { Q("offset", offset), Q("limit", limit) }, cancel);

    public Task<JsonElement?> RemoveHistory(long id) => Call("DELETE", $"/api/history/{id}");

    public Task<JsonElement?> AddToMyList(long id) => Call("PUT", $"/api/watchlist/{id}");

    public Task<JsonElement?> RemoveFromMyList(long id) => Call("DELETE", $"/api/watchlist/{id}");

    public Task<JsonElement?> HideFromContinueWatching(long id) => Call("DELETE", $"/api/continue-watching/{id}");

    /// <summary>The person's rating, 0–10 (halves), or none.</summary>
    public Task<JsonElement?> Rate(long id, double? rating) =>
        Call("PUT", $"/api/items/{id}/rating", rating == null ? new Dictionary<string, object?>() : new Dictionary<string, object?> { ["rating"] = rating });

    public Task<JsonElement?> MarkWatched(long id) => Call("POST", $"/api/items/{id}/watched");

    public Task<JsonElement?> MarkUnwatched(long id) => Call("POST", $"/api/items/{id}/unwatched");

    public Task<JsonElement?> Prefetch(long id) => Call("POST", $"/api/items/{id}/prefetch");

    /// <summary>
    /// The tracks picked for an item (the whole show for an episode). <paramref name="keepAudio"/>
    /// leaves the audio picked before as it is.
    /// </summary>
    public Task<JsonElement?> SetStreams(long itemId, long? fileId, int? audio, int? subtitle, bool keepAudio = false)
    {
        var body = new Dictionary<string, object?>();
        if (fileId != null) body["file_id"] = fileId;
        if (!keepAudio) body["audio_index"] = audio;
        body["subtitle_index"] = subtitle;
        return Call("PUT", $"/api/items/{itemId}/streams", body);
    }

    static bool IsInvalidToken(HttpResponse res)
    {
        var challenge = res.Header("WWW-Authenticate") ?? "";
        // A bare 401 from an older server counts too: the token is what failed.
        return challenge.Length == 0 || InvalidToken.IsMatch(challenge);
    }

    static ApiException ToError(HttpResponse res)
    {
        var code = StatusCode(res.Status);
        var message = $"the server refused (HTTP {res.Status})";
        var body = res.Json();
        string? bodyMessage = null;
        if (body is { ValueKind: JsonValueKind.Object } o)
        {
            code = Json.String(o, "code") ?? code;
            bodyMessage = Json.String(o, "message");
            message = bodyMessage ?? message;
        }
        if (res.Status == 410 && code == "session_stopped") return new SessionStoppedException(bodyMessage);
        return new ApiException(res.Status, code, message, body is { ValueKind: JsonValueKind.Object } ? body : null);
    }

    static string StatusCode(int status) => status switch
    {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        410 => "gone",
        429 => "too_many_requests",
        503 => "unavailable",
        _ => status >= 500 ? "internal" : "bad_request",
    };
}
