using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Auth;

/// <summary>Where small things are kept between launches.</summary>
public interface IKeyValue
{
    string? Get(string key);
    void Set(string key, string value);
    void Remove(string key);
}

/// <summary>The tokens of one server, as kept.</summary>
/// <param name="ExpiresAt">When the access token ends, in ms since the epoch.</param>
public sealed record StoredTokens(
    string Issuer,
    string AccessToken,
    string? RefreshToken,
    long ExpiresAt,
    string Scope,
    string TokenEndpoint,
    string RevocationEndpoint);

/// <summary>A token endpoint's error (RFC 6749 §5.2), with Pley's <c>reason</c> for profile switches.</summary>
public sealed record OAuthError(string Error, string? Description, string? Reason);

public sealed record TokenResponse(string AccessToken, string? RefreshToken, long ExpiresIn, string Scope);

public class TokenException : AuthException
{
    /// <summary>RFC 6749's error codes in words, for a refusal that came without a description (rather than "Invalid_client.").</summary>
    static readonly Dictionary<string, string> Words = new Dictionary<string, string>
    {
        ["invalid_client"] = "the server doesn't know this app",
        ["unauthorized_client"] = "the server doesn't let this app sign in this way",
        ["invalid_grant"] = "the server refused this sign-in",
        ["access_denied"] = "the sign-in was refused",
        ["expired_token"] = "the code expired",
        ["invalid_scope"] = "the server refused what this app asked for",
        ["invalid_request"] = "the server didn't understand the request",
        ["unsupported_grant_type"] = "the server doesn't take this kind of sign-in",
    };

    public TokenException(int status, OAuthError? oauth) : base(MessageOf(status, oauth))
    {
        Status = status;
        OAuth = oauth;
    }

    public int Status { get; }
    public OAuthError? OAuth { get; }

    static string MessageOf(int status, OAuthError? oauth)
    {
        var code = oauth?.Error ?? "";
        if (!string.IsNullOrEmpty(oauth?.Description)) return oauth!.Description!;
        if (Words.TryGetValue(code, out var words)) return words;
        return code.Length > 0 ? $"the server refused ({code.Replace('_', ' ')})" : $"the server refused (HTTP {status})";
    }
}

public class SignedOutException : AuthException
{
    public SignedOutException() : base("you were signed out: sign in again")
    {
    }
}

/// <summary>Why the TV is signed out.</summary>
public enum SignOutReason
{
    Ended,
    WiderScope,
}

/// <summary>The web TV app's <c>auth/tokens.ts</c>: reading and sending token requests.</summary>
public static class TokenRequests
{
    public const string WiderScopeMessage = "the server allowed more than this app asks for: signed out";

    /// <summary>RFC 6749 §5.1: without <c>expires_in</c>, an hour, Pley's own.</summary>
    const long DefaultExpiresIn = 3600;

    const string Unreadable = "the server's answer can't be read: try again";

    public static TokenResponse ParseTokenResponse(JsonElement? body, string askedScope)
    {
        if (body is not { ValueKind: JsonValueKind.Object } r) throw new AuthException(Unreadable);
        var access = Json.String(r, "access_token");
        if (string.IsNullOrEmpty(access)) throw new AuthException(Unreadable);
        var type = Json.String(r, "token_type");
        if (type != null && type.ToLowerInvariant() != "bearer") throw new AuthException(Unreadable);
        var refresh = Json.String(r, "refresh_token");
        var expiresIn = Json.Number(r, "expires_in");
        return new TokenResponse(
            access!,
            string.IsNullOrEmpty(refresh) ? null : refresh,
            expiresIn is > 0 ? (long)expiresIn.Value : DefaultExpiresIn,
            // RFC 6749 §5.1: a scope left out is the one asked for.
            Json.String(r, "scope") ?? askedScope);
    }

    public static OAuthError? ParseOAuthError(JsonElement? body)
    {
        if (body is not { ValueKind: JsonValueKind.Object } r) return null;
        var error = Json.String(r, "error");
        return error == null ? null : new OAuthError(error, Json.String(r, "error_description"), Json.String(r, "reason"));
    }

    /// <summary>The scope names in <paramref name="answer"/> that <paramref name="asked"/> doesn't hold.</summary>
    public static IReadOnlyList<string> ExtraScopes(string answer, string asked)
    {
        var have = asked.Split(' ').Where(s => s.Length > 0).ToList();
        return answer.Split(' ').Where(s => s.Length > 0 && !have.Contains(s)).ToList();
    }

    /// <summary>POSTs <paramref name="fields"/> to a token endpoint; the answer's JSON, or a <see cref="TokenException"/>.</summary>
    public static async Task<JsonElement?> PostForm(Transport http, string url, IEnumerable<KeyValuePair<string, string>> fields)
    {
        var res = await http(new HttpRequest
        {
            Method = "POST",
            Url = url,
            Headers = new Dictionary<string, string> { ["Content-Type"] = "application/x-www-form-urlencoded", ["Accept"] = "application/json" },
            Body = Transports.Form(fields),
            ResponseType = ResponseType.Json,
            TimeoutMs = 15_000,
        }).ConfigureAwait(false);
        if (res.Status < 200 || res.Status >= 300) throw new TokenException(res.Status, ParseOAuthError(res.Json()));
        return res.Json();
    }

    public static StoredTokens Stored(TokenResponse answer, string issuer, string tokenEndpoint, string revocationEndpoint, long now) =>
        new StoredTokens(issuer, answer.AccessToken, answer.RefreshToken, now + answer.ExpiresIn * 1000, answer.Scope, tokenEndpoint, revocationEndpoint);

    /// <summary>Ends the grant of <paramref name="tokens"/> on its server; trouble is ignored (the person can sign the TV out in Account → Apps).</summary>
    public static async Task Revoke(Transport http, string clientId, StoredTokens tokens)
    {
        // The refresh token ends the whole grant; the access token too, when there is no other.
        var token = tokens.RefreshToken ?? tokens.AccessToken;
        try
        {
            await PostForm(http, tokens.RevocationEndpoint, Fields(("token", token), ("client_id", clientId))).ConfigureAwait(false);
        }
        catch (NetworkException)
        {
        }
        catch (TokenException)
        {
        }
    }

    internal static List<KeyValuePair<string, string>> Fields(params (string Key, string Value)[] fields) =>
        fields.Select(f => new KeyValuePair<string, string>(f.Key, f.Value)).ToList();
}

/// <summary>
/// The tokens of one server: the access token while it works, a new pair from the refresh token
/// when it doesn't. Refreshes happen one at a time (the server ends the sign-in when a refresh
/// token is used twice), and the new refresh token is saved before the new access token is used.
/// </summary>
public sealed class Tokens
{
    /// <summary>How long before its end an access token is refreshed.</summary>
    const long EarlyMs = 60_000;

    /// <summary>How long a refreshed token counts as just issued: refused within this, the sign-in is over.</summary>
    public const long JustIssuedMs = 60_000;

    static readonly JsonSerializerOptions JsonOptions = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    readonly SemaphoreSlim queue = new SemaphoreSlim(1, 1);
    readonly IKeyValue store;
    readonly Transport http;
    readonly Action<SignOutReason> onSignedOut;
    readonly Func<long> clock;
    /// <summary>The access token the last refresh or profile switch gave, and when.</summary>
    (string Token, long At)? lastIssued;

    public Tokens(string issuer, string clientId, IKeyValue store, Transport http, Action<SignOutReason>? onSignedOut = null, Func<long>? clock = null)
    {
        Issuer = issuer;
        ClientId = clientId;
        this.store = store;
        this.http = http;
        this.onSignedOut = onSignedOut ?? (_ => { });
        this.clock = clock ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public string Issuer { get; }
    public string ClientId { get; }

    string Key => $"pley.tokens.{Issuer}";

    public StoredTokens? Load()
    {
        var text = store.Get(Key);
        if (string.IsNullOrEmpty(text)) return null;
        try
        {
            var o = JsonSerializer.Deserialize<StoredTokens>(text!, JsonOptions);
            return o != null && !string.IsNullOrEmpty(o.AccessToken) && o.Issuer == Issuer ? o : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    void Save(StoredTokens tokens) => store.Set(Key, JsonSerializer.Serialize(tokens, JsonOptions));

    void Wipe() => store.Remove(Key);

    public bool SignedIn => Load() != null;

    public IReadOnlyList<string> Scopes() => (Load()?.Scope ?? "").Split(' ').Where(s => s.Length > 0).ToList();

    /// <summary>Runs <paramref name="work"/> after every earlier refresh or switch: one at a time.</summary>
    async Task<T> Serial<T>(Func<Task<T>> work)
    {
        await queue.WaitAsync().ConfigureAwait(false);
        try
        {
            return await work().ConfigureAwait(false);
        }
        finally
        {
            queue.Release();
        }
    }

    Task Serial(Func<Task> work) => Serial(async () =>
    {
        await work().ConfigureAwait(false);
        return true;
    });

    /// <summary>The access token to send, refreshed when it ends within a minute; null when signed out.</summary>
    public async Task<string?> AccessToken()
    {
        var tokens = Load();
        if (tokens == null) return null;
        if (tokens.ExpiresAt - clock() > EarlyMs) return tokens.AccessToken;
        return await Refresh(tokens.AccessToken).ConfigureAwait(false);
    }

    /// <summary>
    /// A new access token in place of <paramref name="stale"/> (the one the server refused, or that
    /// is about to end). When another caller refreshed meanwhile, its token is used and nothing is sent.
    /// </summary>
    public Task<string> Refresh(string? stale) => Serial(async () =>
    {
        var tokens = Load() ?? throw new SignedOutException();
        if (tokens.AccessToken != stale && tokens.ExpiresAt - clock() > EarlyMs) return tokens.AccessToken;
        if (tokens.RefreshToken == null) throw SignedOut();
        JsonElement? answer;
        try
        {
            answer = await TokenRequests.PostForm(http, tokens.TokenEndpoint, TokenRequests.Fields(
                ("grant_type", "refresh_token"),
                ("refresh_token", tokens.RefreshToken),
                ("client_id", ClientId))).ConfigureAwait(false);
        }
        catch (TokenException e) when (e.OAuth?.Error == "invalid_grant")
        {
            throw SignedOut();
        }
        var parsed = TokenRequests.ParseTokenResponse(answer, tokens.Scope);
        var fresh = TokenRequests.Stored(parsed, Issuer, tokens.TokenEndpoint, tokens.RevocationEndpoint, clock());
        if (TokenRequests.ExtraScopes(parsed.Scope, tokens.Scope).Count > 0) throw await Widened(fresh).ConfigureAwait(false);
        // A refresh token is good once: the new one is kept before anything else.
        Save(fresh);
        lastIssued = (fresh.AccessToken, clock());
        return fresh.AccessToken;
    });

    /// <summary>
    /// The server answered invalid_token to <paramref name="sent"/>. When it is the token a refresh
    /// gave moments ago, another refresh won't do better: the sign-in is over, and this answers
    /// true. False when <paramref name="sent"/> is older.
    /// </summary>
    public Task<bool> RefusedAfterRefresh(string sent) => Serial(() =>
    {
        var last = lastIssued;
        if (last == null || last.Value.Token != sent || clock() - last.Value.At > JustIssuedMs) return Task.FromResult(false);
        if (Load()?.AccessToken != sent) return Task.FromResult(false);
        End(SignOutReason.Ended);
        return Task.FromResult(true);
    });

    /// <summary>
    /// Moves the sign-in to another profile of the household (the switch-profile grant of Pley's
    /// own apps), or checks the PIN of the one it acts as. A wrong PIN changes nothing.
    /// </summary>
    public Task SwitchProfile(long profile, string? pin) => Serial(async () =>
    {
        var tokens = Load() ?? throw new SignedOutException();
        if (tokens.RefreshToken == null) throw SignedOut();
        var fields = TokenRequests.Fields(
            ("grant_type", Metadata.SwitchProfileGrant),
            ("client_id", ClientId),
            ("refresh_token", tokens.RefreshToken),
            ("profile", profile.ToString(System.Globalization.CultureInfo.InvariantCulture)));
        if (pin != null) fields.Add(new KeyValuePair<string, string>("pin", pin));
        JsonElement? answer;
        try
        {
            answer = await TokenRequests.PostForm(http, tokens.TokenEndpoint, fields).ConfigureAwait(false);
        }
        // Without a reason the refresh token itself is spent or unknown.
        catch (TokenException e) when (e.OAuth?.Error == "invalid_grant" && e.OAuth.Reason == null)
        {
            throw SignedOut();
        }
        var parsed = TokenRequests.ParseTokenResponse(answer, tokens.Scope);
        var fresh = TokenRequests.Stored(parsed, Issuer, tokens.TokenEndpoint, tokens.RevocationEndpoint, clock());
        if (TokenRequests.ExtraScopes(parsed.Scope, tokens.Scope).Count > 0) throw await Widened(fresh).ConfigureAwait(false);
        Save(fresh);
        lastIssued = (fresh.AccessToken, clock());
    });

    /// <summary>Ends this app's sign-in on the server (RFC 7009), then forgets the tokens whatever it answered.</summary>
    public Task SignOut() => Serial(async () =>
    {
        var tokens = Load();
        try
        {
            if (tokens != null) await TokenRequests.Revoke(http, ClientId, tokens).ConfigureAwait(false);
        }
        finally
        {
            Wipe();
        }
    });

    /// <summary>Keeps the tokens of a sign-in that just finished, after the scope guard; the sign-in they replace on this server is ended.</summary>
    public Task Install(StoredTokens fresh, string askedScope) => Serial(async () =>
    {
        if (fresh.Issuer != Issuer) throw new InvalidOperationException("tokens of another server");
        if (TokenRequests.ExtraScopes(fresh.Scope, askedScope).Count > 0)
        {
            await TokenRequests.Revoke(http, ClientId, fresh).ConfigureAwait(false);
            throw new AuthException(TokenRequests.WiderScopeMessage);
        }
        var previous = Load();
        Save(fresh);
        if (previous != null && previous.RefreshToken != fresh.RefreshToken) await TokenRequests.Revoke(http, ClientId, previous).ConfigureAwait(false);
    });

    SignedOutException SignedOut()
    {
        End(SignOutReason.Ended);
        return new SignedOutException();
    }

    void End(SignOutReason reason)
    {
        Wipe();
        lastIssued = null;
        onSignedOut(reason);
    }

    /// <summary>A refresh or switch gave more than this sign-in holds: its grant is ended and the app signed out.</summary>
    async Task<AuthException> Widened(StoredTokens fresh)
    {
        await TokenRequests.Revoke(http, ClientId, fresh).ConfigureAwait(false);
        Wipe();
        onSignedOut(SignOutReason.WiderScope);
        return new AuthException(TokenRequests.WiderScopeMessage);
    }
}
