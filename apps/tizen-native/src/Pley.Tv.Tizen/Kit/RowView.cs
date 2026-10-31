using System;
using System.Collections.Generic;
using Pley.Tv.Core.Ui;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// A row of tiles (the Android app's TvRow, design spec §2.4, D-pad rules 2 to 5): a header, then
/// the tiles, whose focused tile's left edge stays at the gutter (x = 96) while the row moves under
/// it, unless every tile fits on screen unscrolled. It is one focus group that comes back to the
/// tile focused last; Left from its first tile enters the rail, Right stops at its end. While a key
/// repeats it glides, faster the longer the key is held.
/// </summary>
sealed class RowView : View
{
    /// <summary>Pictures are asked for this many tiles past the screen's edge, so they're there when the row moves.</summary>
    const int PictureLead = 3;

    readonly TileKind kind;
    readonly KeyRepeatGate gate;
    readonly View strip;
    readonly List<TileView> tiles = new List<TileView>();
    Animation? scroll;

    public RowView(string? title, IReadOnlyList<TileModel> models, TileKind kind, KeyRepeatGate gate, Images images,
        Action<TileModel> onOpen, Action<TileModel>? onActions, Func<bool> toRail, Action<TileModel>? onFocus = null)
    {
        this.kind = kind;
        this.gate = gate;
        Group = new FocusGroup(Axis.Row, title);
        // D-pad rule 3: Left from the first tile goes to the rail's current item; Right stops at the end.
        Group.Exit = d => d == Direction.Right || (d == Direction.Left && toRail());
        var top = 0f;
        if (title != null)
        {
            var header = Text.Label(title, PleyType.Section, PleyColors.Fg, width: PleyTv.CanvasWidth - PleyTv.Gutter - PleyTv.SafeHorizontal);
            header.Position = new Position(Ui.Dp(PleyTv.Gutter), 0);
            Add(header);
            top = Ui.Dp(PleyTv.RowHeaderHeight + PleyTv.HeaderToTiles);
        }
        strip = new View { Position = new Position(Ui.Dp(PleyTv.Gutter), top) };
        Add(strip);
        var step = Ui.Dp(kind.Width + kind.Gap);
        for (var i = 0; i < models.Count; i++)
        {
            var model = models[i];
            var item = Group.Add(new FocusItem($"{title}/{model.Id}"));
            item.Selected = () => onOpen(model);
            if (onActions != null)
            {
                item.LongSelected = () => onActions(model);
                item.Menu = () => onActions(model);
            }
            var tile = new TileView(model, kind, item, gate, images) { Position = new Position(i * step, 0) };
            if (onFocus != null) item.FocusChanged += on => { if (on) onFocus(model); };
            tiles.Add(tile);
            strip.Add(tile);
        }
        Group.ChildFocused += ScrollTo;
        Size = new Size(Ui.Dp(PleyTv.CanvasWidth), top + Ui.Dp(kind.Height + PleyTv.TilesToCaption + PleyTv.CaptionHeight));
        LoadPictures(0);
    }

    public FocusGroup Group { get; }

    /// <summary>
    /// Where the strip sits for the tile at <paramref name="index"/>: its left edge at the gutter,
    /// or as far as the row goes; a row whose tiles all fit stays where it starts.
    /// </summary>
    float OffsetFor(int index)
    {
        var step = Ui.Dp(kind.Width + kind.Gap);
        var content = tiles.Count * step - Ui.Dp(kind.Gap);
        var room = Ui.Dp(PleyTv.CanvasWidth - PleyTv.Gutter - PleyTv.SafeHorizontal);
        if (content <= room) return 0;
        // The row's end padding (the safe area and a tile) lets the last tile come in from the edge.
        var most = content + Ui.Dp(PleyTv.SafeHorizontal + kind.Width) - Ui.Dp(PleyTv.CanvasWidth - PleyTv.Gutter);
        return -Math.Min(index * step, Math.Max(0, most));
    }

    void ScrollTo(int index)
    {
        var x = Ui.Dp(PleyTv.Gutter) + OffsetFor(index);
        scroll?.Stop();
        scroll = Scrolling.Glide(gate);
        scroll.AnimateTo(strip, "PositionX", x);
        scroll.Play();
        LoadPictures(index);
    }

    /// <summary>Asks for the pictures of the tiles on screen with focus at <paramref name="index"/>, and a few past them.</summary>
    void LoadPictures(int index)
    {
        var step = Ui.Dp(kind.Width + kind.Gap);
        var onScreen = (int)Math.Ceiling(Ui.Dp(PleyTv.CanvasWidth) / step);
        var first = Math.Max(0, (int)(-OffsetFor(index) / step) - 1);
        var last = Math.Min(tiles.Count - 1, first + onScreen + PictureLead);
        for (var i = first; i <= last; i++) tiles[i].LoadPicture();
    }
}

/// <summary>
/// A page of rows (the Android app's TvRowColumn, design spec §2.4, §4.1): rows stacked with a 28 dp
/// gap; the focused row's header comes to y = 172, and while a key repeats the page glides, faster
/// the longer the key is held. Rows scrolled above the focused one aren't drawn, leaving the page's own
/// header (or backdrop) clear.
/// </summary>
sealed class RowColumnView : View
{
    readonly KeyRepeatGate gate;
    readonly float headerY;
    readonly bool firstAtTop;
    readonly View content;
    readonly List<RowView> rows = new List<RowView>();
    Animation? scroll;

    /// <param name="firstAtTop">The first row stays where the page put it (a page with a header); false: it comes to the line too (Home, whose hero goes as the rows come up).</param>
    public RowColumnView(KeyRepeatGate gate, float top, float headerY = PleyTv.FocusedRowHeaderY, bool firstAtTop = true)
    {
        this.gate = gate;
        this.headerY = headerY;
        this.firstAtTop = firstAtTop;
        Size = new Size(Ui.Dp(PleyTv.CanvasWidth), Ui.Dp(PleyTv.CanvasHeight));
        content = new View { PositionY = Ui.Dp(top) };
        Top = Ui.Dp(top);
        Add(content);
        Group = new FocusGroup(Axis.Column, "rows");
        Group.ChildFocused += ScrollTo;
    }

    public FocusGroup Group { get; }

    /// <summary>Where the first row starts while the page is at its top, in pixels.</summary>
    public float Top { get; }

    public void Add(RowView row)
    {
        var y = 0f;
        foreach (var r in rows) y += r.SizeHeight + Ui.Dp(PleyTv.RowGap);
        row.PositionY = y;
        rows.Add(row);
        content.Add(row);
        Group.Add(row.Group);
    }

    void ScrollTo(int index)
    {
        // The first row stays where the page put it; others bring their header to the line.
        var target = index == 0 && firstAtTop ? Top : Ui.Dp(headerY) - rows[index].PositionY;
        scroll?.Stop();
        scroll = Scrolling.Glide(gate);
        scroll.AnimateTo(content, "PositionY", target);
        scroll.Play();
        // Once the page has scrolled, nothing is drawn above the focused row's header (less a
        // little room): the rows above it go, leaving the page's header or backdrop clear.
        for (var i = 0; i < rows.Count; i++)
        {
            if (index > 0 && i < index) rows[i].Hide();
            else rows[i].Show();
        }
    }

    /// <summary>Back to the page's top: the first row where the page put it, every row drawn.</summary>
    public void ScrollToTop()
    {
        scroll?.Stop();
        scroll = new Animation(PleyMotion.FocusMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.EaseOut) };
        scroll.AnimateTo(content, "PositionY", Top);
        scroll.Play();
        foreach (var r in rows) r.Show();
    }
}

/// <summary>How rows and pages move to their focused tile or row.</summary>
static class Scrolling
{
    /// <summary>
    /// A move's animation: eased over 150 ms for a press; for a held key, evenly over a little more
    /// than the time to the next move (<see cref="KeyRepeatGate.GlideMs"/>), so each step runs into
    /// the next without a pause and the row glides, faster the longer the key is held.
    /// </summary>
    public static Animation Glide(KeyRepeatGate gate) => gate.Holding
        ? new Animation(gate.GlideMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.Linear) }
        : new Animation(PleyMotion.FocusMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.EaseOut) };
}
