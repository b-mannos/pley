using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using Pley.Tv.Core.Net;

namespace Pley.Tv.Core.Auth;

/// <summary>A sign-in problem, in a few words for people.</summary>
public class AuthException : Exception
{
    public AuthException(string message) : base(message)
    {
    }
}

/// <summary>The server didn't answer at all: worth trying again by itself.</summary>
public class UnreachableException : AuthException
{
    public UnreachableException(string message = "the server can't be reached: check its address and that it's on") : base(message)
    {
    }
}

/// <summary>
/// Where a server signs apps in (RFC 8414), read from
/// <c>&lt;server&gt;/.well-known/oauth-authorization-server</c>. Only kept when it is this
/// server's own: its issuer is the address the app used and every endpoint is on it, so an
/// answer can't send the app's codes or tokens anywhere else.
/// </summary>
public sealed record ServerMetadata(
    string Issuer,
    string TokenEndpoint,
    string RevocationEndpoint,
    string? DeviceAuthorizationEndpoint,
    IReadOnlyList<string>? ScopesSupported,
    IReadOnlyList<string>? GrantTypesSupported)
{
    /// <summary>Whether a TV can sign in at this server with a code.</summary>
    [JsonIgnore]
    public bool SignsInWithCodes => DeviceAuthorizationEndpoint != null && (GrantTypesSupported ?? new string[0]).Contains(Metadata.DeviceCodeGrant);
}

/// <summary>The web TV app's <c>auth/metadata.ts</c>.</summary>
public static class Metadata
{
    public const string TooOld = "this server is too old for this app";

    /// <summary>RFC 8628's grant: a TV collects its tokens with the code it was given.</summary>
    public const string DeviceCodeGrant = "urn:ietf:params:oauth:grant-type:device_code";

    /// <summary>Pley's own apps move between the household's profiles with this grant.</summary>
    public const string SwitchProfileGrant = "urn:pley:params:oauth:grant-type:switch-profile";

    /// <summary>What a server without sign-in with a code answers.</summary>
    public const string NoCodes = "this server can't sign in TVs with a code yet: update it";

    public const string Path = "/.well-known/oauth-authorization-server";

    public static async Task<ServerMetadata> Fetch(Transport http, string server)
    {
        var home = Addresses.Origin(server) ?? throw new AuthException("that isn't a server address");
        HttpResponse res;
        try
        {
            res = await http(new HttpRequest
            {
                Url = home + Path,
                Headers = new Dictionary<string, string> { ["Accept"] = "application/json" },
                TimeoutMs = 8000,
            }).ConfigureAwait(false);
        }
        catch (NetworkException)
        {
            throw new UnreachableException();
        }
        if (res.Status == 404) throw new AuthException(TooOld);
        if (res.Status < 200 || res.Status >= 300) throw new AuthException($"the server couldn't answer (HTTP {res.Status})");
        // Older servers answer their web app's page here.
        if ((res.Header("Content-Type") ?? "").ToLowerInvariant().IndexOf("json", StringComparison.Ordinal) < 0) throw new AuthException(TooOld);
        return Parse(res.Text ?? "", home);
    }

    /// <summary>The checks of <see cref="Fetch"/>, on the answer <paramref name="text"/> of the server at <paramref name="server"/>.</summary>
    public static ServerMetadata Parse(string text, string server)
    {
        var home = Addresses.Origin(server) ?? throw new AuthException("that isn't a server address");
        JsonElement o;
        try
        {
            using var doc = JsonDocument.Parse(text);
            o = doc.RootElement.Clone();
        }
        catch (JsonException)
        {
            throw new AuthException(TooOld);
        }
        if (o.ValueKind != JsonValueKind.Object) throw new AuthException(TooOld);
        var issuer = Json.String(o, "issuer") ?? "";
        if (issuer.Length == 0) throw new AuthException(TooOld);
        // RFC 8414 §3.3: the issuer is exactly the address asked at.
        if (issuer != home)
        {
            throw new AuthException(
                $"this server calls itself {(issuer.Length > 100 ? issuer.Substring(0, 100) : issuer)}: open it by that address, or check the proxy in front of it passes Host");
        }
        string Endpoint(string name)
        {
            var url = Json.String(o, name);
            if (string.IsNullOrEmpty(url)) throw new AuthException(TooOld);
            if (Addresses.Origin(url!) != issuer || !url!.StartsWith(issuer + "/", StringComparison.Ordinal))
                throw new AuthException("this server sends apps to another address to sign in: Pley doesn't");
            return url;
        }
        var device = string.IsNullOrEmpty(Json.String(o, "device_authorization_endpoint")) ? null : Endpoint("device_authorization_endpoint");
        return new ServerMetadata(
            issuer,
            Endpoint("token_endpoint"),
            Endpoint("revocation_endpoint"),
            device,
            Json.Strings(o, "scopes_supported"),
            Json.Strings(o, "grant_types_supported"));
    }
}
