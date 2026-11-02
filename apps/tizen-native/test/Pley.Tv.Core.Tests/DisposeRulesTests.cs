using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>
/// The app's views as NUI disposes them. A view collected without being disposed is disposed from
/// NUI's queue on the UI thread, with whatever was collected with it and in no set order: its
/// Dispose calling into another NUI object (an animation stopped, a timer, a child hidden) found
/// it gone at times, and the app closed natively. Such calls are only for a Dispose the app makes
/// (<c>type == DisposeTypes.Explicit</c>); otherwise it only checks Disposed, lets go of events of
/// the Core's objects and calls the base (docs/TV_PLATFORMS.md, On the TV).
/// </summary>
public class DisposeRulesTests
{
    static readonly Regex Override = new Regex(@"override\s+void\s+Dispose\s*\(\s*DisposeTypes\s+(\w+)\s*\)");

    static readonly Regex[] Allowed =
    {
        new Regex(@"^if\s*\(\s*Disposed\s*\)\s*return;$"),
        new Regex(@"^base\.Dispose\(\s*\w+\s*\);$"),
        // An event of the Core's (a focus item's), let go of: plain .NET, no NUI.
        new Regex(@"^(if\s*\(\s*[\w.]+\s*!=\s*null\s*\)\s*)?[\w.]+\s*-=\s*[\w.]+;$"),
    };

    public static IEnumerable<object[]> Overrides() =>
        Directory.EnumerateFiles(Path.Combine(System.AppContext.BaseDirectory, "app"), "*.cs", SearchOption.AllDirectories)
            .SelectMany(file => Bodies(File.ReadAllText(file)).Select(b => new object[] { $"{Path.GetFileName(file)}:{b.Line}", b.Parameter, b.Body }));

    [Fact]
    public void The_app_has_views_with_their_own_dispose()
    {
        Assert.NotEmpty(Overrides());
    }

    [Theory]
    [MemberData(nameof(Overrides))]
    public void A_view_collected_calls_into_no_other_NUI_object(string at, string parameter, string body)
    {
        foreach (var statement in Statements(WithoutExplicit(Uncommented(body), parameter)))
            Assert.True(Allowed.Any(a => a.IsMatch(statement)),
                $"{at}: \"{statement}\" runs when NUI disposes the view from its queue too; only inside if ({parameter} == DisposeTypes.Explicit)");
    }

    [Fact]
    public void A_call_outside_the_guard_is_caught()
    {
        const string body = "{ if (Disposed) return; item.FocusChanged -= Look; slide?.Stop(); base.Dispose(type); }";
        var left = Statements(WithoutExplicit(body, "type")).ToList();
        Assert.Contains("slide?.Stop();", left);
        Assert.DoesNotContain(left, s => !Allowed.Any(a => a.IsMatch(s)) && s != "slide?.Stop();");
    }

    [Fact]
    public void A_call_inside_the_guard_passes()
    {
        const string body = "{ if (Disposed) return; if (type == DisposeTypes.Explicit) { slide?.Stop(); catchUp.Stop(); } if (type == DisposeTypes.Explicit) look?.Stop(); base.Dispose(type); }";
        Assert.All(Statements(WithoutExplicit(body, "type")), s => Assert.Contains(Allowed, a => a.IsMatch(s)));
    }

    /// <summary>Each override's body, braces and all, with the line it starts on and its parameter's name.</summary>
    static IEnumerable<(int Line, string Parameter, string Body)> Bodies(string source)
    {
        foreach (Match m in Override.Matches(source))
        {
            var open = source.IndexOf('{', m.Index + m.Length);
            var end = Closing(source, open);
            yield return (source.Take(m.Index).Count(c => c == '\n') + 1, m.Groups[1].Value, source.Substring(open, end - open + 1));
        }
    }

    /// <summary>Where the brace opened at <paramref name="open"/> closes.</summary>
    static int Closing(string text, int open)
    {
        var depth = 0;
        for (var i = open; i < text.Length; i++)
        {
            if (text[i] == '{') depth++;
            else if (text[i] == '}' && --depth == 0) return i;
        }
        throw new InvalidDataException("a Dispose without its closing brace");
    }

    static string Uncommented(string body) => Regex.Replace(body, @"//[^\n]*|/\*.*?\*/", "", RegexOptions.Singleline);

    /// <summary>The body without what runs only when the app disposes the view: a block or a statement under the guard.</summary>
    static string WithoutExplicit(string body, string parameter)
    {
        var guard = new Regex($@"if\s*\(\s*{Regex.Escape(parameter)}\s*==\s*DisposeTypes\.Explicit\s*\)\s*");
        for (var m = guard.Match(body); m.Success; m = guard.Match(body))
        {
            var after = m.Index + m.Length;
            var end = body[after] == '{' ? Closing(body, after) : body.IndexOf(';', after);
            body = body.Remove(m.Index, end - m.Index + 1);
        }
        return body;
    }

    /// <summary>The body's statements, each with its semicolon, its spaces made single.</summary>
    static IEnumerable<string> Statements(string body)
    {
        var inner = body.Trim();
        inner = inner.Substring(1, inner.Length - 2);
        foreach (var part in inner.Split(';'))
        {
            var statement = Regex.Replace(part, @"\s+", " ").Trim();
            if (statement.Length > 0) yield return statement + ";";
        }
    }
}
