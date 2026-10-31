using System.Collections.Concurrent;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core.Auth;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>Finding a server (the web TV app's "finding a server" tests, and the sweep).</summary>
public class ServersTests
{
    const string Pley = @"{""name"":""Home"",""version"":""0.1.0""}";

    [Fact]
    public void Names_servers_by_host_and_port()
    {
        Assert.Equal("router:7539", Servers.HostPort("http://router:7539"));
        Assert.Equal("pley.example.com", Servers.HostPort("https://pley.example.com"));
        Assert.Equal("192.168.1.", Servers.SubnetOf("192.168.1.42"));
        Assert.Null(Servers.SubnetOf("127.0.0.1"));
        Assert.Null(Servers.SubnetOf(null));
    }

    [Fact]
    public async Task Tries_https_first_for_a_name_on_the_internet_then_pleys_port()
    {
        var server = new FakeServer().On("GET", "http://pley.example.com:7539/api/server", 200, Pley);
        var info = await Servers.Connect(server.Transport, "pley.example.com");
        Assert.Equal(new ServerInfo("http://pley.example.com:7539", "Home", "0.1.0"), info);
        Assert.Equal(new[] { "https://pley.example.com/api/server", "http://pley.example.com:7539/api/server" }, server.Requests.Select(r => r.Url));
    }

    [Fact]
    public async Task Says_what_answered_when_it_isnt_pley_and_why_nothing_did()
    {
        var notPley = await Assert.ThrowsAsync<ConnectException>(() => Servers.Connect(new FakeServer().On("GET", "http://router:7539/api/server", 404).Transport, "router"));
        Assert.Equal(new ConnectProblem.NotPley("router:7539"), notPley.Problem);
        var nothing = await Assert.ThrowsAsync<ConnectException>(() => Servers.Connect(new FakeServer().Transport, "192.168.1.77"));
        Assert.Equal(new ConnectProblem.Unreachable("192.168.1.77:7539", "nothing answers there"), nothing.Problem);
        var bad = await Assert.ThrowsAsync<ConnectException>(() => Servers.Connect(new FakeServer().Transport, "not an address"));
        Assert.IsType<ConnectProblem.BadAddress>(bad.Problem);
        Assert.Equal("Enter an address such as nas, 192.168.1.20 or pley.example.com.", bad.Message);
    }

    [Fact]
    public async Task Names_a_server_without_a_name_by_its_address()
    {
        var server = new FakeServer().On("GET", "http://nas:7539/api/server", 200, @"{""name"":""  "",""version"":""0.1.0""}");
        Assert.Equal("nas", (await Servers.Check(server.Transport, "http://nas:7539")).Name);
    }

    [Fact]
    public async Task Sweeps_the_TVs_network_at_pleys_port_and_reports_what_answers()
    {
        var server = new FakeServer()
            .On("GET", "http://192.168.1.20:7539/api/server", 200, Pley)
            .On("GET", "http://192.168.1.30:7539/api/server", 200, "<html>");
        var found = new ConcurrentBag<ServerInfo>();
        await Servers.Sweep(server.Transport, "192.168.1.", found.Add, CancellationToken.None);
        Assert.Equal(new[] { "http://192.168.1.20:7539" }, found.Select(f => f.Url));
        Assert.Equal(254, server.Requests.Count);
        Assert.All(server.Requests, r => Assert.True(r.TimeoutMs <= Servers.SweepTimeoutMs));
    }
}
