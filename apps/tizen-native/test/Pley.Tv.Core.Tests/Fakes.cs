using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Pley.Tv.Core.Auth;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Tests;

/// <summary>A server in a test: answers by method and URL, and remembers what it was asked.</summary>
sealed class FakeServer
{
    readonly Dictionary<string, Func<HttpRequest, (int Status, string? Body)>> routes = new Dictionary<string, Func<HttpRequest, (int, string?)>>();

    public ConcurrentQueue<HttpRequest> Requests { get; } = new ConcurrentQueue<HttpRequest>();

    /// <summary>Answers <paramref name="method"/> <paramref name="url"/>; a null status throws a network failure.</summary>
    public FakeServer On(string method, string url, Func<HttpRequest, (int Status, string? Body)> answer)
    {
        routes[method + " " + url] = answer;
        return this;
    }

    public FakeServer On(string method, string url, int status, string? body = null) => On(method, url, _ => (status, body));

    public Transport Transport => req =>
    {
        Requests.Enqueue(req);
        if (!routes.TryGetValue(req.Method + " " + req.Url, out var answer)) throw new NetworkException("Can't reach the server");
        var (status, body) = answer(req);
        if (status == 0) throw new NetworkException(Transports.TimedOut);
        return Task.FromResult(new HttpResponse(status, name => name.Equals("Content-Type", StringComparison.OrdinalIgnoreCase) ? "application/json" : null, body));
    };
}

sealed class MemoryKeyValue : IKeyValue
{
    public Dictionary<string, string> Values { get; } = new Dictionary<string, string>();

    public string? Get(string key) => Values.TryGetValue(key, out var v) ? v : null;

    public void Set(string key, string value) => Values[key] = value;

    public void Remove(string key) => Values.Remove(key);
}

/// <summary>Timers that run when a test moves the clock past them; posts run at once.</summary>
sealed class FakeScheduler : IScheduler
{
    readonly List<(long At, Action Action, Cancelled Handle)> timers = new List<(long, Action, Cancelled)>();

    public long Now { get; private set; } = 1_700_000_000_000;

    public IDisposable After(long ms, Action action)
    {
        var handle = new Cancelled();
        timers.Add((Now + ms, action, handle));
        return handle;
    }

    public void Post(Action action) => action();

    /// <summary>Moves the clock by <paramref name="ms"/>, running each timer due on the way, in order.</summary>
    public void Advance(long ms)
    {
        var until = Now + ms;
        while (true)
        {
            var due = timers.Where(t => !t.Handle.Disposed && t.At <= until).OrderBy(t => t.At).FirstOrDefault();
            if (due.Action == null) break;
            timers.Remove(due);
            Now = due.At;
            due.Action();
        }
        Now = until;
    }

    public sealed class Cancelled : IDisposable
    {
        public bool Disposed { get; private set; }

        public void Dispose() => Disposed = true;
    }
}

/// <summary>A clock tests move by hand; sleeping on it moves it at once.</summary>
sealed class FakeClock
{
    public long Now { get; set; } = 1_700_000_000_000;

    public List<long> Sleeps { get; } = new List<long>();

    public Func<long> Read => () => Now;

    public Task Sleep(long ms, System.Threading.CancellationToken cancel)
    {
        Sleeps.Add(ms);
        Now += ms;
        cancel.ThrowIfCancellationRequested();
        return Task.CompletedTask;
    }
}
