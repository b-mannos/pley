using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Nodes;
using Pley.Tv.Core.Auth;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>Sign-in answers are read as the web TV app reads them (fixtures/auth.json).</summary>
public class AuthFixtureTests
{
    static readonly JsonSerializerOptions Camel = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    public static IEnumerable<object[]> Indexes(string rule) =>
        Enumerable.Range(0, Fixtures.Load("auth").GetProperty(rule).GetArrayLength()).Select(i => new object[] { i });

    public static IEnumerable<object[]> MetadataCases => Indexes("metadata");
    public static IEnumerable<object[]> TokenCases => Indexes("tokenResponse");
    public static IEnumerable<object[]> ErrorCases => Indexes("oauthError");
    public static IEnumerable<object[]> DeviceCases => Indexes("deviceCode");

    static JsonElement Case(string rule, int i) => Fixtures.Load("auth").GetProperty(rule)[i];

    /// <summary>What happened, written as the fixture writes it: { ok: answer } or { error: message }.</summary>
    static JsonNode? Outcome(Func<object?> f)
    {
        try
        {
            return new JsonObject { ["ok"] = JsonSerializer.SerializeToNode(f(), Camel) };
        }
        catch (AuthException e)
        {
            return new JsonObject { ["error"] = e.Message };
        }
    }

    static void Same(JsonElement expected, JsonNode? actual) =>
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse(expected.GetRawText()), actual), $"expected {expected.GetRawText()}\n  actual {actual?.ToJsonString()}");

    [Theory, MemberData(nameof(MetadataCases))]
    public void Metadata(int i)
    {
        var c = Case("metadata", i);
        var input = c.GetProperty("in");
        Same(c.GetProperty("out"), Outcome(() => Auth.Metadata.Parse(input.GetProperty("text").GetString()!, input.GetProperty("server").GetString()!)));
    }

    [Theory, MemberData(nameof(TokenCases))]
    public void TokenResponse(int i)
    {
        var c = Case("tokenResponse", i);
        var input = c.GetProperty("in");
        Same(c.GetProperty("out"), Outcome(() => TokenRequests.ParseTokenResponse(input.GetProperty("body"), input.GetProperty("asked").GetString()!)));
    }

    [Theory, MemberData(nameof(ErrorCases))]
    public void OAuthError(int i)
    {
        var c = Case("oauthError", i);
        Same(c.GetProperty("out"), JsonSerializer.SerializeToNode(TokenRequests.ParseOAuthError(c.GetProperty("in")), Camel));
    }

    [Theory, MemberData(nameof(DeviceCases))]
    public void DeviceCode(int i)
    {
        var c = Case("deviceCode", i);
        var first = Case("metadata", 0).GetProperty("in");
        var metadata = Auth.Metadata.Parse(first.GetProperty("text").GetString()!, first.GetProperty("server").GetString()!);
        Same(c.GetProperty("out"), Outcome(() => DeviceSignIn.ParseDeviceCode(c.GetProperty("in"), "https://pley.example.com", metadata, "library play", 1_700_000_000_000)));
    }
}
