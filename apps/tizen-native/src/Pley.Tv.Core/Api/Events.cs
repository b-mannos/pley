using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Auth;

namespace Pley.Tv.Core.Api;

/// <summary>
/// One of the server's events (docs/API.md, "Activity &amp; events"). Types: library_changed,
/// libraries_changed, sessions_changed, watchlist_changed, user_state_changed, session_stopped,
/// scan_started, scan_progress, scan_finished, live_channels_changed, live_tuners_changed.
/// </summary>
public sealed record ServerEvent
{
    public string Type { get; init; } = "";
    public long? LibraryId { get; init; }
    /// <summary>user_state_changed: the item, then its parent and grandparent.</summary>
    public IReadOnlyList<long>? ItemIds { get; init; }
    /// <summary>user_state_changed: everything under the first item changed too.</summary>
    public bool? Children { get; init; }
    public string? SessionId { get; init; }
    public string? Message { get; init; }
    public bool? ByAdmin { get; init; }
}

/// <summary>
/// Opens the event stream at <paramref name="url"/> with <paramref name="token"/>: calls
/// <paramref name="onOpen"/> once the server answers 200, <paramref name="onText"/> with each
/// piece of the body as it arrives, and answers the status once the stream ends.
/// </summary>
public delegate Task<int> EventConnect(string url, string token, Action onOpen, Action<string> onText, CancellationToken cancel);

/// <summary>Reads Server-Sent Events from text as it arrives: each event's data lines, joined.</summary>
public sealed class SseReader
{
    static readonly Regex Blank = new Regex(@"\r?\n\r?\n", RegexOptions.CultureInvariant);
    static readonly Regex Line = new Regex(@"\r?\n", RegexOptions.CultureInvariant);
    string buffer = "";

    /// <summary>The events <paramref name="text"/> completes.</summary>
    public IReadOnlyList<ServerEvent> Feed(string text)
    {
        buffer += text;
        var blocks = Blank.Split(buffer);
        buffer = blocks[blocks.Length - 1];
        var events = new List<ServerEvent>();
        foreach (var block in blocks.Take(blocks.Length - 1))
        {
            var data = string.Join("\n", Line.Split(block)
                .Where(l => l.StartsWith("data:", StringComparison.Ordinal))
                .Select(l => l.Substring(5).StartsWith(" ") ? l.Substring(6) : l.Substring(5)));
            if (data.Length == 0) continue;
            try
            {
                var e = JsonSerializer.Deserialize<ServerEvent>(data, ApiJson.Options);
                if (e != null && e.Type.Length > 0) events.Add(e);
            }
            catch (JsonException)
            {
                // Not an event this app reads.
            }
        }
        return events;
    }
}

/// <summary>
/// The server's events (<c>GET /api/events</c>), with the token in the Authorization header,
/// the only place Pley takes an app's token. Reconnects as the Android app's EventStream does;
/// tells <c>onReconnect</c> on every connection, since nothing is replayed. The web TV app's
/// <c>api/events.ts</c>.
/// </summary>
public sealed class EventStream
{
    /// <summary>First wait after a stream fails, doubled each time up to the last.</summary>
    public const long FirstRetryMs = 2_000;
    public const long MaxRetryMs = 30_000;
    /// <summary>A stream that ran this long and then ended ended as planned (its token expired): it reconnects at once.</summary>
    public const long StableMs = 30_000;
    /// <summary>A stream that runs for hours: a fresh one starts rather than read on forever.</summary>
    const long MaxRead = 4 * 1024 * 1024;

    readonly string server;
    readonly Tokens tokens;
    readonly Action<ServerEvent> onEvent;
    readonly Action onReconnect;
    readonly EventConnect connect;
    readonly IScheduler scheduler;
    CancellationTokenSource? running;
    IDisposable? timer;
    int failures;
    bool stopped = true;

    public EventStream(string server, Tokens tokens, Action<ServerEvent> onEvent, Action onReconnect, EventConnect connect, IScheduler scheduler)
    {
        this.server = server;
        this.tokens = tokens;
        this.onEvent = onEvent;
        this.onReconnect = onReconnect;
        this.connect = connect;
        this.scheduler = scheduler;
    }

    public void Start()
    {
        if (!stopped) return;
        stopped = false;
        _ = Connect();
    }

    public void Stop()
    {
        stopped = true;
        timer?.Dispose();
        timer = null;
        running?.Cancel();
        running = null;
    }

    /// <summary>The server answers again: a stream waiting out its back-off connects now. False when none was waiting.</summary>
    public bool RetryNow()
    {
        if (stopped || running != null || timer == null) return false;
        timer.Dispose();
        timer = null;
        failures = 0;
        _ = Connect();
        return true;
    }

    long RetryDelay()
    {
        var wait = FirstRetryMs;
        for (var i = 1; i < failures; i++) wait = Math.Min(wait * 2, MaxRetryMs);
        return Math.Min(wait, MaxRetryMs);
    }

    void Later(long ms)
    {
        if (stopped) return;
        timer = scheduler.After(ms, () =>
        {
            timer = null;
            _ = Connect();
        });
    }

    async Task Connect()
    {
        if (stopped) return;
        string? token;
        try
        {
            token = await tokens.AccessToken();
        }
        catch (Exception)
        {
            token = null;
        }
        if (token == null || stopped)
        {
            failures++;
            Later(RetryDelay());
            return;
        }
        var mine = new CancellationTokenSource();
        running = mine;
        long openedAt = -1;
        long read = 0;
        var reader = new SseReader();
        int status;
        try
        {
            // The stream is read off the UI thread; what it says is told there, in order.
            status = await connect($"{server}/api/events", token, () => scheduler.Post(() =>
            {
                if (openedAt >= 0 || running != mine) return;
                openedAt = scheduler.Now;
                failures = 0;
                onReconnect();
            }), text =>
            {
                var events = reader.Feed(text);
                read += text.Length;
                if (read > MaxRead) mine.Cancel();
                if (events.Count > 0) scheduler.Post(() =>
                {
                    if (stopped) return;
                    foreach (var e in events) onEvent(e);
                });
            }, mine.Token);
        }
        catch (Exception)
        {
            status = 0;
        }
        if (running != mine) return;
        running = null;
        if (stopped) return;
        if (status == 401)
        {
            try
            {
                await tokens.Refresh(token);
                Later(0);
            }
            catch (Exception)
            {
                failures++;
                Later(RetryDelay());
            }
            return;
        }
        if (status == 403) return;
        if (openedAt >= 0 && scheduler.Now - openedAt >= StableMs)
        {
            Later(0);
            return;
        }
        failures++;
        Later(RetryDelay());
    }

    static readonly HttpClient Client = new HttpClient { Timeout = System.Threading.Timeout.InfiniteTimeSpan };

    /// <summary>The event stream over HttpClient, read as it arrives.</summary>
    public static readonly EventConnect Http = async (url, token, onOpen, onText, cancel) =>
    {
        var req = new HttpRequestMessage(HttpMethod.Get, url);
        req.Headers.TryAddWithoutValidation("Accept", "text/event-stream");
        req.Headers.TryAddWithoutValidation("Authorization", "Bearer " + token);
        try
        {
            using var res = await Client.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, cancel).ConfigureAwait(false);
            var status = (int)res.StatusCode;
            if (status != 200) return status;
            onOpen();
            using var stream = await res.Content.ReadAsStreamAsync().ConfigureAwait(false);
            using var text = new StreamReader(stream, Encoding.UTF8);
            var buffer = new char[8 * 1024];
            int n;
            while ((n = await text.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false)) > 0)
            {
                cancel.ThrowIfCancellationRequested();
                onText(new string(buffer, 0, n));
            }
            return status;
        }
        catch (OperationCanceledException)
        {
            return 0;
        }
        catch (HttpRequestException)
        {
            return 0;
        }
        catch (IOException)
        {
            return 0;
        }
    };
}
