using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Auth;

/// <summary>A Pley server that answered at <paramref name="Url"/>.</summary>
public sealed record ServerInfo(string Url, string Name, string Version);

/// <summary>Why an address isn't a server to sign in to.</summary>
public abstract record ConnectProblem
{
    public sealed record BadAddress : ConnectProblem;

    public sealed record NotPley(string Address) : ConnectProblem;

    public sealed record Unreachable(string Address, string Reason) : ConnectProblem;

    /// <summary>The Android app's words for each problem (TvServersScreen's problemText).</summary>
    public string Text => this switch
    {
        BadAddress => "Enter an address such as nas, 192.168.1.20 or pley.example.com.",
        NotPley p => $"Something answers at {p.Address}, but it isn't Pley.",
        Unreachable u => $"Couldn't reach {u.Address}: {u.Reason}",
        _ => "",
    };
}

public class ConnectException : Exception
{
    public ConnectException(ConnectProblem problem) : base(problem.Text) => Problem = problem;

    public ConnectProblem Problem { get; }
}

/// <summary>Finding and checking servers: the web TV app's <c>auth/servers.ts</c>.</summary>
public static class Servers
{
    /// <summary>How many addresses a sweep asks at once, how long each may take, and how long it may run.</summary>
    public const int SweepParallel = 48;
    public const int SweepTimeoutMs = 1_500;
    public const int SweepTotalMs = 6_000;

    static readonly Regex HostOfUrl = new Regex(@"^(https?)://([^/]+)", RegexOptions.CultureInvariant);
    static readonly Regex HostForTimeout = new Regex(@"^https?://(\[[^\]]+\]|[^/:]+)", RegexOptions.CultureInvariant);
    static readonly Regex Dotted = new Regex(@"^([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\z", RegexOptions.CultureInvariant);

    /// <summary><c>host:port</c> as the server list shows it, never the scheme ("192.168.1.20:7539", "pley.example.com").</summary>
    public static string HostPort(string url)
    {
        var m = HostOfUrl.Match(url);
        return m.Success ? m.Groups[2].Value : url;
    }

    static int TimeoutFor(string url)
    {
        var m = HostForTimeout.Match(url);
        return Addresses.IsLocalHost(m.Success ? m.Groups[1].Value : "") ? 3_000 : 10_000;
    }

    /// <summary>
    /// Asks <paramref name="url"/> whether it is Pley (GET /api/server, no token): its name and
    /// version, null when something else answered. Network trouble throws <see cref="NetworkException"/>.
    /// </summary>
    public static async Task<ServerInfo?> Probe(Transport http, string url, int? timeoutMs = null, CancellationToken cancel = default)
    {
        var res = await http(new HttpRequest
        {
            Url = url + "/api/server",
            Headers = new Dictionary<string, string> { ["Accept"] = "application/json" },
            ResponseType = ResponseType.Json,
            TimeoutMs = timeoutMs ?? TimeoutFor(url),
            Cancel = cancel,
        }).ConfigureAwait(false);
        if (res.Status != 200) return null;
        if (res.Json() is not { ValueKind: JsonValueKind.Object } body) return null;
        var version = Json.String(body, "version");
        if (version == null) return null;
        var name = Json.String(body, "name")?.Trim();
        return new ServerInfo(url, string.IsNullOrEmpty(name) ? Addresses.DisplayAddress(url) : name!, version);
    }

    static string ReasonOf(NetworkException e) => e.Message.Contains("too long") ? "no answer in time" : "nothing answers there";

    /// <summary>
    /// The server someone typed (the address pad's Connect): each candidate in turn, https first
    /// for names on the internet; the first Pley answer wins. Throws <see cref="ConnectException"/>
    /// with why none did.
    /// </summary>
    public static async Task<ServerInfo> Connect(Transport http, string input)
    {
        var list = Addresses.Candidates(input);
        if (list.Count == 0) throw new ConnectException(new ConnectProblem.BadAddress());
        string? notPley = null;
        (string Address, string Reason)? last = null;
        foreach (var url in list)
        {
            try
            {
                var info = await Probe(http, url).ConfigureAwait(false);
                if (info != null) return info;
                notPley ??= HostPort(url);
            }
            catch (NetworkException e)
            {
                last = (HostPort(url), ReasonOf(e));
            }
        }
        if (notPley != null) throw new ConnectException(new ConnectProblem.NotPley(notPley));
        throw new ConnectException(new ConnectProblem.Unreachable(last?.Address ?? input, last?.Reason ?? "nothing answers there"));
    }

    /// <summary>A server already known (a row): it answers as Pley, or why not.</summary>
    public static async Task<ServerInfo> Check(Transport http, string url)
    {
        try
        {
            return await Probe(http, url).ConfigureAwait(false) ?? throw new ConnectException(new ConnectProblem.NotPley(HostPort(url)));
        }
        catch (NetworkException e)
        {
            throw new ConnectException(new ConnectProblem.Unreachable(HostPort(url), ReasonOf(e)));
        }
    }

    /// <summary>The /24 of an IPv4 address without its last number ("192.168.1."); null for anything else.</summary>
    public static string? SubnetOf(string? address)
    {
        var m = Dotted.Match(address ?? "");
        if (!m.Success || m.Groups[1].Value == "0" || m.Groups[1].Value == "127") return null;
        return $"{m.Groups[1].Value}.{m.Groups[2].Value}.{m.Groups[3].Value}.";
    }

    /// <summary>
    /// "Search this network": GET /api/server on every address of the TV's /24 at Pley's port, a
    /// few dozen at a time, for at most 6 s. Servers are reported as they answer.
    /// </summary>
    public static async Task Sweep(Transport http, string subnet, Action<ServerInfo> onFound, CancellationToken stop, Func<long>? now = null)
    {
        now ??= () => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var started = now();
        var next = 0;
        async Task Worker()
        {
            while (!stop.IsCancellationRequested && now() - started < SweepTotalMs)
            {
                var host = Interlocked.Increment(ref next);
                if (host > 254) return;
                var url = $"http://{subnet}{host}:{Addresses.DefaultPort}";
                try
                {
                    var left = (int)Math.Max(1, SweepTotalMs - (now() - started));
                    var info = await Probe(http, url, Math.Min(SweepTimeoutMs, left), stop).ConfigureAwait(false);
                    if (info != null && !stop.IsCancellationRequested) onFound(info);
                }
                catch (NetworkException)
                {
                    // Nothing there.
                }
            }
        }
        await Task.WhenAll(Enumerable.Range(0, SweepParallel).Select(_ => Worker())).ConfigureAwait(false);
    }

    /// <summary>Whether two addresses are the same server.</summary>
    public static bool SameServer(string a, string b) => Addresses.Origin(a) == Addresses.Origin(b);
}
