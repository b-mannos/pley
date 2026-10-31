using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Pley.Tv.Core.Net;

/// <summary>How a response's body is read.</summary>
public enum ResponseType
{
    Text,
    /// <summary>Text, parsed as JSON on demand (<see cref="HttpResponse.Json"/>).</summary>
    Json,
    Bytes,
}

/// <summary>One HTTP request (the web TV app's <c>net/transport.ts</c>).</summary>
public sealed record HttpRequest
{
    public string Method { get; init; } = "GET";
    public string Url { get; init; } = "";
    public IReadOnlyDictionary<string, string>? Headers { get; init; }
    public string? Body { get; init; }
    public ResponseType ResponseType { get; init; } = ResponseType.Text;
    public int TimeoutMs { get; init; } = Transports.DefaultTimeoutMs;
    /// <summary>Stops the request: a screen that went away, a newer search.</summary>
    public CancellationToken Cancel { get; init; }
    /// <summary>Hears how much of the body has arrived so far, as it arrives (a speed test).</summary>
    public Action<long>? OnProgress { get; init; }
}

public sealed class HttpResponse
{
    readonly Func<string, string?> header;
    bool parsed;
    JsonElement? json;

    public HttpResponse(int status, Func<string, string?> header, string? text = null, byte[]? bytes = null)
    {
        Status = status;
        this.header = header;
        Text = text;
        Bytes = bytes;
    }

    public int Status { get; }
    public string? Text { get; }
    public byte[]? Bytes { get; }

    /// <summary>A response header, case-insensitively; null when absent.</summary>
    public string? Header(string name) => header(name);

    /// <summary>The body as JSON; null when it is empty or isn't JSON.</summary>
    public JsonElement? Json()
    {
        if (parsed) return json;
        parsed = true;
        if (string.IsNullOrEmpty(Text)) return null;
        try
        {
            using var doc = JsonDocument.Parse(Text!);
            json = doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            json = null;
        }
        return json;
    }
}

/// <summary>Sends requests: HttpClient on the TV, a fake in tests.</summary>
public delegate Task<HttpResponse> Transport(HttpRequest request);

/// <summary>No answer: the network, a timeout, or a request cancelled.</summary>
public class NetworkException : Exception
{
    public NetworkException(string message, bool cancelled = false) : base(message) => Cancelled = cancelled;

    public bool Cancelled { get; }
}

public static class Transports
{
    /// <summary>A request's timeout, as its error says it.</summary>
    public const string TimedOut = "The server took too long to answer";

    /// <summary>Requests of more than this wait as long as they need: segments, images over a slow network.</summary>
    public const int DefaultTimeoutMs = 20_000;

    static readonly HttpClient Client = new HttpClient(new HttpClientHandler { AllowAutoRedirect = true })
    {
        // Each request has its own timeout.
        Timeout = System.Threading.Timeout.InfiniteTimeSpan,
    };

    /// <summary>HttpClient, with each request's timeout and cancellation.</summary>
    public static readonly Transport Http = async req =>
    {
        if (req.Cancel.IsCancellationRequested) throw new NetworkException("cancelled", true);
        using var timeout = new CancellationTokenSource(req.TimeoutMs);
        using var both = CancellationTokenSource.CreateLinkedTokenSource(timeout.Token, req.Cancel);
        var message = new HttpRequestMessage(new HttpMethod(req.Method), req.Url);
        string? contentType = null;
        foreach (var h in req.Headers ?? new Dictionary<string, string>())
        {
            if (h.Key.Equals("Content-Type", StringComparison.OrdinalIgnoreCase)) contentType = h.Value;
            else message.Headers.TryAddWithoutValidation(h.Key, h.Value);
        }
        if (req.Body != null)
        {
            message.Content = new StringContent(req.Body, Encoding.UTF8);
            if (contentType != null) message.Content.Headers.ContentType = System.Net.Http.Headers.MediaTypeHeaderValue.Parse(contentType);
        }
        try
        {
            using var res = await Client.SendAsync(message, HttpCompletionOption.ResponseHeadersRead, both.Token).ConfigureAwait(false);
            var bytes = await ReadBody(res, req.OnProgress, both.Token).ConfigureAwait(false);
            var headers = res.Headers.Concat(res.Content.Headers).ToList();
            string? Header(string name) => headers.Where(h => h.Key.Equals(name, StringComparison.OrdinalIgnoreCase))
                .Select(h => string.Join(", ", h.Value)).FirstOrDefault();
            return req.ResponseType == ResponseType.Bytes
                ? new HttpResponse((int)res.StatusCode, Header, bytes: bytes)
                : new HttpResponse((int)res.StatusCode, Header, text: Encoding.UTF8.GetString(bytes));
        }
        catch (OperationCanceledException) when (req.Cancel.IsCancellationRequested)
        {
            throw new NetworkException("cancelled", true);
        }
        catch (OperationCanceledException)
        {
            throw new NetworkException(TimedOut);
        }
        catch (HttpRequestException)
        {
            throw new NetworkException("Can't reach the server");
        }
        catch (IOException)
        {
            throw new NetworkException("Can't reach the server");
        }
    };

    static async Task<byte[]> ReadBody(HttpResponseMessage res, Action<long>? onProgress, CancellationToken cancel)
    {
        using var stream = await res.Content.ReadAsStreamAsync().ConfigureAwait(false);
        using var memory = new MemoryStream();
        var buffer = new byte[64 * 1024];
        int n;
        while ((n = await stream.ReadAsync(buffer, 0, buffer.Length, cancel).ConfigureAwait(false)) > 0)
        {
            memory.Write(buffer, 0, n);
            onProgress?.Invoke(memory.Length);
        }
        return memory.ToArray();
    }

    /// <summary><c>application/x-www-form-urlencoded</c> of <paramref name="fields"/>, in order.</summary>
    public static string Form(IEnumerable<KeyValuePair<string, string>> fields) =>
        string.Join("&", fields.Select(f => Uri.EscapeDataString(f.Key) + "=" + Uri.EscapeDataString(f.Value)));

    /// <summary><paramref name="path"/> with <paramref name="query"/> added, values left out when null.</summary>
    public static string WithQuery(string path, IEnumerable<KeyValuePair<string, object?>>? query)
    {
        if (query == null) return path;
        var parts = query.Where(q => q.Value != null).Select(q => Uri.EscapeDataString(q.Key) + "=" + Uri.EscapeDataString(QueryValue(q.Value!))).ToList();
        if (parts.Count == 0) return path;
        return path + (path.IndexOf('?') >= 0 ? "&" : "?") + string.Join("&", parts);
    }

    // As JavaScript's String() writes them: true and false in lower case, numbers without a culture.
    static string QueryValue(object value) => value switch
    {
        bool b => b ? "true" : "false",
        IFormattable f => f.ToString(null, System.Globalization.CultureInfo.InvariantCulture),
        _ => value.ToString() ?? "",
    };
}
