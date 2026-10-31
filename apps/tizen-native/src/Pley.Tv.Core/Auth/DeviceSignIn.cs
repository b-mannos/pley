using System;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Auth;

/// <summary>
/// A code for signing in a TV (RFC 8628): shown as <see cref="UserCode"/> with
/// <see cref="VerificationUri"/> and a QR code of <see cref="VerificationUriComplete"/>, and
/// approved on a phone or computer at the server's /link page.
/// </summary>
/// <param name="DeviceCodeValue">What the TV polls with. Never shown.</param>
/// <param name="UserCode">As shown: <c>BCDF-GHJK</c>.</param>
/// <param name="ExpiresAt">When the code stops working, in ms since the epoch.</param>
public sealed record DeviceCode(
    string Server,
    string Issuer,
    [property: JsonPropertyName("deviceCode")] string DeviceCodeValue,
    string UserCode,
    string VerificationUri,
    string? VerificationUriComplete,
    long ExpiresAt,
    int IntervalSeconds,
    string TokenEndpoint,
    string RevocationEndpoint,
    string Scope);

/// <summary>Where a code stands.</summary>
public abstract record DevicePoll
{
    public sealed record Waiting : DevicePoll;

    public sealed record SignedIn(StoredTokens Tokens) : DevicePoll;

    public sealed record Denied : DevicePoll;

    public sealed record Expired : DevicePoll;

    /// <summary>The server can't be reached: polling goes on by itself in <paramref name="RetryInMs"/>.</summary>
    public sealed record Unreachable(long RetryInMs) : DevicePoll;

    public sealed record Failed(string Message) : DevicePoll;
}

/// <summary>
/// The device authorization grant (RFC 8628 §3) against a Pley server: POST client_id and scope
/// to the metadata's device_authorization_endpoint for a code, then ask the token endpoint with
/// it until someone answers. The web TV app's <c>auth/device.ts</c>.
/// </summary>
public sealed class DeviceSignIn
{
    /// <summary>Everything Pley's own TV apps may ask for (docs/API.md, "Scopes").</summary>
    public static readonly string[] AllScopes = { "library", "play", "playlists", "profiles" };

    /// <summary>RFC 8628 §3.2: without an interval, 5 seconds.</summary>
    public const int DefaultIntervalSeconds = 5;

    /// <summary>What a slow_down adds to this wait and every later one (§3.5).</summary>
    public const long SlowDownMs = 5_000;

    /// <summary>The waits after network failures in a row.</summary>
    public static readonly long[] BackoffMs = { 2_000, 4_000, 8_000, 16_000, 30_000 };

    readonly Transport http;
    readonly string clientId;
    readonly Func<long> clock;
    readonly Func<long, CancellationToken, Task> sleep;

    public DeviceSignIn(Transport http, string clientId, Func<long>? clock = null, Func<long, CancellationToken, Task>? sleep = null)
    {
        this.http = http;
        this.clientId = clientId;
        this.clock = clock ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        this.sleep = sleep ?? ((ms, cancel) => Task.Delay(TimeSpan.FromMilliseconds(ms), cancel));
    }

    /// <summary>The scope to ask for: what the server offers of Pley's own.</summary>
    public static string ScopeFor(ServerMetadata metadata)
    {
        var offered = metadata.ScopesSupported;
        return string.Join(" ", offered == null ? AllScopes : AllScopes.Where(offered.Contains));
    }

    public async Task<DeviceCode> Start(string server)
    {
        var home = Addresses.Origin(server) ?? throw new AuthException("that isn't a server address");
        if (Addresses.IsUnencrypted(home)) throw new AuthException(Addresses.Unencrypted);
        var metadata = await Metadata.Fetch(http, home).ConfigureAwait(false);
        if (!metadata.SignsInWithCodes) throw new AuthException(Metadata.NoCodes);
        var scope = ScopeFor(metadata);
        JsonElement? answer;
        try
        {
            answer = await TokenRequests.PostForm(http, metadata.DeviceAuthorizationEndpoint!, TokenRequests.Fields(
                ("client_id", clientId),
                ("scope", scope))).ConfigureAwait(false);
        }
        catch (TokenException e) when (e.OAuth?.Error is "invalid_client" or "unauthorized_client")
        {
            // A server from before Pley's TV apps were built in.
            throw new AuthException("this server doesn't know Pley's TV apps yet: update it");
        }
        catch (TokenException e) when (e.Status == 429)
        {
            throw new AuthException("too many TVs are signing in here right now: try again in a minute");
        }
        catch (NetworkException)
        {
            throw new UnreachableException();
        }
        return ParseDeviceCode(answer, home, metadata, scope, clock());
    }

    /// <summary>
    /// Asks whether <paramref name="code"/> was approved, every interval (slower when the server
    /// says so), until it is signed in, denied or expired; <paramref name="onUpdate"/> hears each
    /// change. Stops early, answering Waiting, when <paramref name="stop"/> is cancelled.
    /// </summary>
    public async Task<DevicePoll> Poll(DeviceCode code, Action<DevicePoll> onUpdate, CancellationToken stop)
    {
        long interval = (code.IntervalSeconds > 0 ? code.IntervalSeconds : DefaultIntervalSeconds) * 1000L;
        var failures = 0;
        var troubled = false;
        onUpdate(new DevicePoll.Waiting());
        DevicePoll Done(DevicePoll state)
        {
            onUpdate(state);
            return state;
        }
        long Backoff() => Math.Max(interval, BackoffMs[Math.Min(failures, BackoffMs.Length) - 1]);
        while (true)
        {
            var wait = failures == 0 ? interval : Backoff();
            try
            {
                await sleep(Math.Min(wait, Math.Max(0, code.ExpiresAt - clock())), stop).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return new DevicePoll.Waiting();
            }
            if (stop.IsCancellationRequested) return new DevicePoll.Waiting();
            if (clock() >= code.ExpiresAt) return Done(new DevicePoll.Expired());
            DevicePoll? outcome = null;
            try
            {
                var answer = await TokenRequests.PostForm(http, code.TokenEndpoint, TokenRequests.Fields(
                    ("grant_type", Metadata.DeviceCodeGrant),
                    ("device_code", code.DeviceCodeValue),
                    ("client_id", clientId))).ConfigureAwait(false);
                failures = 0;
                var parsed = TokenRequests.ParseTokenResponse(answer, code.Scope);
                outcome = new DevicePoll.SignedIn(TokenRequests.Stored(parsed, code.Issuer, code.TokenEndpoint, code.RevocationEndpoint, clock()));
            }
            catch (TokenException e)
            {
                failures = 0;
                switch (e.OAuth?.Error)
                {
                    case "authorization_pending":
                        break;
                    case "slow_down":
                        interval += SlowDownMs;
                        break;
                    case "access_denied":
                        outcome = new DevicePoll.Denied();
                        break;
                    case "expired_token":
                        outcome = new DevicePoll.Expired();
                        break;
                    case "invalid_grant":
                        outcome = new DevicePoll.Failed("this code doesn't work anymore: get a new one");
                        break;
                    default:
                        outcome = new DevicePoll.Failed(e.Message);
                        break;
                }
            }
            catch (NetworkException)
            {
                failures++;
            }
            catch (AuthException e)
            {
                outcome = new DevicePoll.Failed(e.Message);
            }
            if (stop.IsCancellationRequested) return new DevicePoll.Waiting();
            if (outcome != null) return Done(outcome);
            if (failures > 0)
            {
                troubled = true;
                onUpdate(new DevicePoll.Unreachable(Backoff()));
            }
            else if (troubled)
            {
                troubled = false;
                onUpdate(new DevicePoll.Waiting());
            }
        }
    }

    /// <summary>The code in <paramref name="body"/>, the answer of <paramref name="server"/>'s device authorization endpoint (§3.2).</summary>
    public static DeviceCode ParseDeviceCode(JsonElement? body, string server, ServerMetadata metadata, string scope, long now)
    {
        var bad = new AuthException("the server's answer can't be read: try again");
        if (body is not { ValueKind: JsonValueKind.Object } o) throw bad;
        var deviceCode = Json.String(o, "device_code");
        var userCode = Json.String(o, "user_code");
        var uri = Json.String(o, "verification_uri");
        if (string.IsNullOrEmpty(deviceCode) || string.IsNullOrEmpty(userCode) || uri == null) throw bad;
        // The page to open must be this server's own: a code typed elsewhere would sign someone else's TV in.
        if (Addresses.Origin(uri) != metadata.Issuer) throw new AuthException("this server sends people to another address to approve TVs: Pley doesn't");
        var complete = Json.String(o, "verification_uri_complete");
        if (complete != null && Addresses.Origin(complete) != metadata.Issuer) complete = null;
        var expiresIn = Json.Number(o, "expires_in") is > 0 and var e ? e : 600;
        var interval = Json.Number(o, "interval") is > 0 and var i ? i : DefaultIntervalSeconds;
        return new DeviceCode(
            server,
            metadata.Issuer,
            deviceCode!,
            userCode!,
            uri,
            complete,
            now + (long)(expiresIn * 1000),
            (int)interval,
            metadata.TokenEndpoint,
            metadata.RevocationEndpoint,
            scope);
    }
}
