using System.Collections.Generic;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>Server addresses answer as the web TV app's do (fixtures/addresses.json).</summary>
public class AddressesTests
{
    public static IEnumerable<object[]> NormalizeServerCases => Fixtures.Cases("addresses", "normalizeServer");
    public static IEnumerable<object[]> CandidatesCases => Fixtures.Cases("addresses", "candidates");
    public static IEnumerable<object[]> IsLocalHostCases => Fixtures.Cases("addresses", "isLocalHost");
    public static IEnumerable<object[]> AtHomeCases => Fixtures.Cases("addresses", "atHome");
    public static IEnumerable<object[]> UnencryptedCases => Fixtures.Cases("addresses", "unencrypted");
    public static IEnumerable<object[]> DisplayAddressCases => Fixtures.Cases("addresses", "displayAddress");
    public static IEnumerable<object[]> OriginCases => Fixtures.Cases("addresses", "origin");

    [Theory, MemberData(nameof(NormalizeServerCases))]
    public void NormalizeServer(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.NormalizeServer(input)));

    [Theory, MemberData(nameof(CandidatesCases))]
    public void Candidates(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.Candidates(input)));

    [Theory, MemberData(nameof(IsLocalHostCases))]
    public void IsLocalHost(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.IsLocalHost(input)));

    [Theory, MemberData(nameof(AtHomeCases))]
    public void AtHome(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.AtHome(input)));

    [Theory, MemberData(nameof(UnencryptedCases))]
    public void Unencrypted(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.IsUnencrypted(input)));

    [Theory, MemberData(nameof(DisplayAddressCases))]
    public void DisplayAddress(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.DisplayAddress(input)));

    [Theory, MemberData(nameof(OriginCases))]
    public void Origin(string input, string expected) =>
        Assert.Equal(Fixtures.Normalize(expected), Fixtures.Json(Addresses.Origin(input)));
}
