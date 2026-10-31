using System;
using System.Collections.Generic;
using System.Linq;
using Pley.Tv.Core.Api;
using Pley.Tv.Core.Live;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Screens;

/// <summary>
/// The programme guide (the Android app's TvGuide): what has focus described above the grid,
/// channels down and time across, now marked by a line. Focus is one item, moved by the guide
/// itself (<see cref="GuideCursor"/>) rather than by geometry, so only the rows in view have
/// views, however many channels there are. It opens on the channel last watched, when it is in
/// the category.
/// </summary>
sealed class GuideView : View
{
    const float TileWidth = 120, TileHeight = 68, TileGap = 24;

    readonly LiveTvModel model;
    readonly Images images;
    readonly KeyRepeatGate gate;
    readonly LiveCategory listed;
    readonly string origin;
    readonly Func<bool> toCategories;
    readonly Action<LiveChannel> watch;
    readonly Action<LiveChannel, LiveProgram?> details;
    readonly GuideCursor cursor;
    readonly LiveClock clock = LiveClock.Local;
    readonly float width;
    readonly float rowsHeight;
    readonly float timeWidth;

    readonly View info = new View();
    readonly TextLabel title;
    readonly TextLabel meta;
    readonly TextLabel words;
    View? badge;
    ChannelLogoView? tile;
    long? tileChannel;

    readonly TextLabel day;
    readonly View headerLayer = new View();
    readonly View rowsLayer = new View();
    readonly View timeLayer = new View();
    readonly View rowsTimeLayer = new View();
    readonly View nowLine;
    readonly Dictionary<int, GuideChannelCell> cells = new Dictionary<int, GuideChannelCell>();
    readonly Dictionary<int, (ProgramsLook Look, View View)> rows = new Dictionary<int, (ProgramsLook, View)>();

    ChannelPages pages = ChannelPages.Empty;
    GuidePrograms guide = GuidePrograms.None;
    long now;
    int top;
    long drawnLeft;
    Landing? placed;
    Animation? slide;

    /// <param name="widthDp">From the guide's left edge to the right safe edge.</param>
    /// <param name="toCategories">Left from the guide's start: focus to the category shown.</param>
    public GuideView(LiveTvModel model, Images images, KeyRepeatGate gate, LiveCategory listed, string origin, float widthDp, float heightDp,
        Func<bool> toCategories, Action<LiveChannel> watch, Action<LiveChannel, LiveProgram?> details)
    {
        this.model = model;
        this.images = images;
        this.gate = gate;
        this.listed = listed;
        this.origin = origin;
        this.toCategories = toCategories;
        this.watch = watch;
        this.details = details;
        width = widthDp;
        timeWidth = widthDp - GuideLayout.ChannelWidth;
        var rowsTop = GuideLayout.InfoHeight + GuideLayout.HeaderHeight;
        rowsHeight = heightDp - rowsTop;
        now = model.Now.Value;
        cursor = new GuideCursor(model.GuideStart, now);
        drawnLeft = cursor.Left;
        Size = new Size(Ui.Dp(widthDp), Ui.Dp(heightDp));

        // What has focus: its title, channel and times, and what the guide says of it; the channel's tile at the right.
        var textWidth = widthDp - TileWidth - TileGap;
        title = Text.Label("", PleyType.Section, PleyColors.Fg, width: textWidth);
        title.PositionY = Ui.Dp(4);
        info.Add(title);
        meta = Text.Label("", PleyType.Label, PleyColors.Muted, width: textWidth);
        meta.PositionY = Ui.Dp(4 + PleyType.Section.Line + 2);
        info.Add(meta);
        words = Text.Label("", PleyType.Body, PleyColors.Muted, maxLines: 2, width: Math.Min(560, textWidth));
        words.VerticalAlignment = VerticalAlignment.Top;
        words.PositionY = Ui.Dp(4 + PleyType.Section.Line + 2 + PleyType.Label.Line + 6);
        info.Add(words);
        Add(info);

        // The times across the top, the day at the left.
        day = Text.Label("", PleyType.Caption, PleyColors.Fg, width: GuideLayout.ChannelWidth - GuideLayout.ChannelInset);
        day.Position = new Position(Ui.Dp(GuideLayout.ChannelInset), Ui.Dp(GuideLayout.InfoHeight + (GuideLayout.HeaderHeight - PleyType.Caption.Line) / 2));
        Add(day);
        var header = new View
        {
            Position = new Position(Ui.Dp(GuideLayout.ChannelWidth), Ui.Dp(GuideLayout.InfoHeight)),
            Size = new Size(Ui.Dp(timeWidth), Ui.Dp(GuideLayout.HeaderHeight)),
            ClippingMode = ClippingModeType.ClipChildren,
        };
        header.Add(headerLayer);
        Add(header);

        // The rows: the channels' cells, and over them their programmes in time, which slide as the grid moves.
        var grid = new View
        {
            Position = new Position(0, Ui.Dp(rowsTop)),
            Size = new Size(Ui.Dp(widthDp), Ui.Dp(rowsHeight)),
            ClippingMode = ClippingModeType.ClipChildren,
        };
        grid.Add(rowsLayer);
        var times = new View
        {
            Position = new Position(Ui.Dp(GuideLayout.ChannelWidth), 0),
            Size = new Size(Ui.Dp(timeWidth), Ui.Dp(rowsHeight)),
            ClippingMode = ClippingModeType.ClipChildren,
        };
        timeLayer.Add(rowsTimeLayer);
        nowLine = new View
        {
            Size = new Size(Ui.Dp(GuideLayout.NowLineWidth), Ui.Dp(rowsHeight)),
            BackgroundColor = LiveViews.Alpha(PleyColors.Accent, 0.85f),
        };
        timeLayer.Add(nowLine);
        times.Add(timeLayer);
        grid.Add(times);
        Add(grid);
        timeLayer.PositionX = -Ui.Dp(GuideLayout.Scroll(cursor.Left, cursor.Start));
        headerLayer.PositionX = timeLayer.PositionX;

        Item = new FocusItem("guide")
        {
            Selected = Ok,
            OwnKey = OnKey,
        };
        Item.LongSelected = Details;
        Item.Menu = Details;
        Item.FocusChanged += _ => Draw();
        model.ShowingTimes(cursor.Left, cursor.Left + GuideLayout.ViewS);
        Header();
    }

    /// <summary>The guide's one focus item.</summary>
    public FocusItem Item { get; }

    int Total => pages.Total ?? 0;

    public void SetPages(ChannelPages value)
    {
        pages = value;
        cursor.Fit(Total);
        ShowingRows();
        Draw();
    }

    public void SetPrograms(GuidePrograms value)
    {
        guide = value;
        Draw();
    }

    public void SetNow(long value)
    {
        now = value;
        Draw();
    }

    /// <summary>The list opens at the channel last watched (once per landing).</summary>
    public void Land(Landing? landing)
    {
        if (landing == null || landing.Category != listed || placed == landing) return;
        placed = landing;
        cursor.Place(landing.Index ?? 0, Total);
        Scroll(animate: false);
        Draw();
    }

    /// <summary>Back away from now: the guide comes back to now first.</summary>
    public bool Back()
    {
        if (!Item.Focused || pages[cursor.Row] == null || !cursor.AwayFromNow(Programs(cursor.Row), now)) return false;
        cursor.BackToNow(now);
        Moved();
        return true;
    }

    IReadOnlyList<LiveProgram>? Programs(int row) => pages[row] is { } c ? guide.Of(c.Id) : null;

    bool OnKey(string key, bool down)
    {
        if (Disposed) return false;
        var programs = Programs(cursor.Row);
        switch (key)
        {
            case "Left":
                if (!down) return true;
                if (!cursor.MoveLeft(programs)) Scrolling.Left(gate, toCategories);
                else Moved();
                return true;
            case "Right":
                if (!down) return true;
                cursor.MoveRight(programs);
                Moved();
                return true;
            case "Up":
                if (!down) return true;
                // Up from the first row leaves for the tabs.
                if (!cursor.MoveUp()) return false;
                RowMoved();
                return true;
            case "Down":
                if (!down) return true;
                cursor.MoveDown(Total);
                RowMoved();
                return true;
        }
        if (RemoteKeys.ChannelUp(key) || RemoteKeys.ChannelDown(key))
        {
            if (!down) return true;
            if (RemoteKeys.ChannelUp(key)) cursor.PageUp();
            else cursor.PageDown(Total);
            RowMoved();
            return true;
        }
        if (RemoteKeys.Info(key))
        {
            if (!down) Details();
            return true;
        }
        return false;
    }

    /// <summary>OK on a programme on now (or a channel without guide) watches it; on another, its details.</summary>
    void Ok()
    {
        var channel = pages[cursor.Row];
        var programs = Programs(cursor.Row);
        switch (cursor.Ok(channel, programs, now))
        {
            case GuideOk.Watch:
                watch(channel!);
                break;
            case GuideOk.Details:
                details(channel!, cursor.Current(programs));
                break;
        }
    }

    /// <summary>Holding OK, Menu or Info: the details of the programme with focus.</summary>
    void Details()
    {
        if (pages[cursor.Row] is { } channel && cursor.Details(Programs(cursor.Row)) is { } program) details(channel, program);
    }

    /// <summary>The cursor moved in time: the grid follows by half hours, its programmes asked for.</summary>
    void Moved()
    {
        if (cursor.Left != drawnLeft)
        {
            drawnLeft = cursor.Left;
            model.ShowingTimes(cursor.Left, cursor.Left + GuideLayout.ViewS);
            Header();
            slide?.Stop();
            slide = new Animation(PleyMotion.OverlayMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.EaseOut) };
            var x = -Ui.Dp(GuideLayout.Scroll(cursor.Left, cursor.Start));
            slide.AnimateTo(timeLayer, "PositionX", x);
            slide.AnimateTo(headerLayer, "PositionX", x);
            slide.Play();
        }
        Draw();
    }

    void RowMoved()
    {
        Scroll(animate: true);
        Draw();
    }

    /// <summary>The rows scroll when the focused one reaches the first or the last in view, bringing it second.</summary>
    void Scroll(bool animate)
    {
        var shown = (int)Math.Ceiling(rowsHeight / GuideLayout.RowHeight);
        var row = cursor.Row;
        if (row <= top || row >= top + shown - 1) top = Math.Max(0, row - 1);
        // The last row stays at the foot, above the room under it.
        var full = (int)((rowsHeight - GuideLayout.BottomPadding) / GuideLayout.RowHeight);
        top = Math.Max(0, Math.Min(top, Total - full));
        var y = -Ui.Dp(top * GuideLayout.RowHeight);
        if (animate)
        {
            var a = Scrolling.Glide(gate);
            a.AnimateTo(rowsLayer, "PositionY", y);
            a.AnimateTo(rowsTimeLayer, "PositionY", y);
            a.Play();
        }
        else
        {
            rowsLayer.PositionY = y;
            rowsTimeLayer.PositionY = y;
        }
        ShowingRows();
    }

    void ShowingRows()
    {
        var shown = (int)Math.Ceiling(rowsHeight / GuideLayout.RowHeight);
        model.ShowingRows(top, top + shown);
    }

    /// <summary>The half hours across the top, a screen either way of the grid's edge, in time as the programmes are.</summary>
    void Header()
    {
        foreach (var child in headerLayer.Children.ToList())
        {
            headerLayer.Remove(child);
            child.Dispose();
        }
        foreach (var (time, x) in GuideLayout.Slots(cursor.Left, cursor.Start))
        {
            var midnight = clock.Midnight(time);
            var label = Text.Label(midnight ? clock.Day(time, now) : clock.Time(time), PleyType.Caption, midnight ? PleyColors.Fg : PleyColors.Muted, width: 100);
            label.Position = new Position(Ui.Dp(x + 6), Ui.Dp((GuideLayout.HeaderHeight - PleyType.Caption.Line) / 2));
            headerLayer.Add(label);
        }
    }

    /// <summary>Draws what changed: the focused programme's description, and the rows in view (and one either side).</summary>
    void Draw()
    {
        if (Disposed) return;
        var focused = Item.Focused;
        var channel = pages[cursor.Row];
        var programs = Programs(cursor.Row);
        var current = channel != null ? cursor.Current(programs) : null;
        Info(channel, current);
        Text.Show(day, clock.Day(Math.Max(cursor.Left, cursor.Start), now));
        nowLine.PositionX = Ui.Dp(GuideLayout.X(now, cursor.Start) - GuideLayout.NowLineWidth / 2);

        var shown = (int)Math.Ceiling(rowsHeight / GuideLayout.RowHeight);
        var first = Math.Max(0, top - 1);
        var last = Math.Min(Total - 1, top + shown + 1);
        foreach (var i in cells.Keys.Where(i => i < first || i > last).ToList())
        {
            cells[i].Dispose();
            cells.Remove(i);
        }
        foreach (var i in rows.Keys.Where(i => i < first || i > last).ToList())
        {
            rows[i].View.Dispose();
            rows.Remove(i);
        }
        for (var i = first; i <= last; i++)
        {
            var c = pages[i];
            var rowFocused = focused && i == cursor.Row;
            if (!cells.TryGetValue(i, out var cell) || !Equals(cell.Channel, c))
            {
                cell?.Dispose();
                cell = new GuideChannelCell(c, c != null ? LiveRules.ChannelName(c, listed) : "", origin, images, width);
                cell.PositionY = Ui.Dp(i * GuideLayout.RowHeight);
                rowsLayer.Add(cell);
                cells[i] = cell;
            }
            cell.Focus(rowFocused);
            var list = c != null ? guide.Of(c.Id) : null;
            var look = new ProgramsLook(c, list, guide.Loaded, cursor.Left, now, rowFocused ? cursor.Current(list)?.Start ?? -1 : null);
            if (rows.TryGetValue(i, out var drawn) && drawn.Look == look) continue;
            drawn.View?.Dispose();
            var view = ProgramsRow(look);
            view.PositionY = Ui.Dp(i * GuideLayout.RowHeight);
            rowsTimeLayer.Add(view);
            rows[i] = (look, view);
        }
    }

    void Info(LiveChannel? channel, LiveProgram? program)
    {
        if (channel == null)
        {
            info.Hide();
            return;
        }
        info.Show();
        var name = LiveRules.ChannelName(channel, listed);
        badge?.Dispose();
        badge = program != null ? LiveViews.BadgeOf(program) : null;
        var x = 0f;
        if (badge != null)
        {
            badge.Position = new Position(0, Ui.Dp(4) + (Text.LineHeight(PleyType.Section) - badge.SizeHeight) / 2);
            info.Add(badge);
            x = badge.SizeWidth + Ui.Dp(8);
        }
        Text.Show(title, program?.Title ?? name);
        title.PositionX = x;
        title.SizeWidth = Ui.Dp(width - TileWidth - TileGap) - x;
        Text.Show(meta, LiveText.GuideMeta(channel, name, program, now, clock));
        words.Text = LiveText.GuideText(program) ?? "";
        if (tileChannel != channel.Id || tile == null)
        {
            tile?.Dispose();
            tile = new ChannelLogoView(channel, origin, images, TileWidth, TileHeight, name: true);
            tile.Position = new Position(Ui.Dp(width - TileWidth), Ui.Dp(4));
            info.Add(tile);
            tileChannel = channel.Id;
        }
    }

    /// <summary>What a row of programmes shows: drawn again only when it changes.</summary>
    readonly record struct ProgramsLook(LiveChannel? Channel, IReadOnlyList<LiveProgram>? Programs, bool Loaded, long Left, long Now, long? Focus);

    /// <summary>A channel's programmes in time, x from the guide's start: the focused one in the accent, what airs marked, what ended dimmed.</summary>
    View ProgramsRow(ProgramsLook look)
    {
        var start = cursor.Start;
        var row = new View { Size = new Size(Ui.Dp(GuideLayout.X(start + LiveRules.GuideSpanS, start)), Ui.Dp(GuideLayout.RowHeight)) };
        var leftX = GuideLayout.X(look.Left, start);
        var cellHeight = GuideLayout.RowHeight - 2 * GuideLayout.CellInsetY;
        if (look.Channel == null)
        {
            row.Add(new View
            {
                Position = new Position(Ui.Dp(leftX + GuideLayout.CellInsetX), Ui.Dp(GuideLayout.CellInsetY)),
                Size = new Size(Ui.Dp(timeWidth - 2 * GuideLayout.CellInsetX), Ui.Dp(cellHeight)),
                CornerRadius = Ui.Dp(PleyShape.ControlRadius),
                BackgroundColor = PleyColors.Surface2,
            });
            return row;
        }
        var visible = GuideLayout.Visible(look.Programs, look.Left, start);
        if (visible.Count == 0)
        {
            if (!look.Loaded) return row;
            // Nothing in view: the channel says why, across the grid.
            var on = look.Focus == -1;
            var filler = new View
            {
                Position = new Position(Ui.Dp(leftX + GuideLayout.CellInsetX), Ui.Dp(GuideLayout.CellInsetY)),
                Size = new Size(Ui.Dp(timeWidth - 2 * GuideLayout.CellInsetX), Ui.Dp(cellHeight)),
                CornerRadius = Ui.Dp(PleyShape.ControlRadius),
                BackgroundColor = on ? PleyColors.Accent : LiveViews.Alpha(PleyColors.Surface, 0.6f),
            };
            var label = Text.Label(look.Channel.HasGuide ? LiveText.NoInfo : LiveRules.ChannelName(look.Channel, listed), PleyType.Caption, on ? PleyColors.OnAccent : PleyColors.Subtle,
                width: timeWidth - 2 * GuideLayout.CellInsetX - 24);
            label.Position = new Position(Ui.Dp(12), Ui.Dp((cellHeight - PleyType.Caption.Line) / 2));
            filler.Add(label);
            row.Add(filler);
            return row;
        }
        foreach (var program in visible)
        {
            var (x, w) = GuideLayout.Cell(program, start);
            var focused = look.Focus == program.Start;
            var airing = program.AiringAt(look.Now);
            var cellWidth = Math.Max(0, w - 2 * GuideLayout.CellInsetX);
            var cell = new View
            {
                Position = new Position(Ui.Dp(x + GuideLayout.CellInsetX), Ui.Dp(GuideLayout.CellInsetY)),
                Size = new Size(Ui.Dp(cellWidth), Ui.Dp(cellHeight)),
                CornerRadius = Ui.Dp(PleyShape.ControlRadius),
                BackgroundColor = focused ? PleyColors.Accent : airing ? LiveViews.PrimaryContainer : PleyColors.Surface2,
                ClippingMode = ClippingModeType.ClipChildren,
                Opacity = program.Stop <= look.Now && !focused ? GuideLayout.PastAlpha : 1f,
            };
            if (airing && !focused)
            {
                cell.BorderlineWidth = Ui.Dp(1);
                cell.BorderlineColor = LiveViews.Alpha(PleyColors.Accent, 0.35f);
                cell.BorderlineOffset = -1;
            }
            if (w > GuideLayout.MinTextWidth)
            {
                // A programme begun left of the grid keeps its title in view.
                var offset = Math.Max(0, Math.Min(leftX - x, Math.Max(0, w - GuideLayout.TitleRoom)));
                var textWidth = Math.Max(10, cellWidth - offset - 2 * GuideLayout.CellPadding);
                var name = Text.Label(program.Title, PleyType.Label, focused ? PleyColors.OnAccent : PleyColors.Fg, width: textWidth);
                if (airing || focused) name.FontStyle = Text.Weight(PleyType.Bold);
                var block = PleyType.Label.Line + PleyType.Caption.Line;
                name.Position = new Position(Ui.Dp(offset + GuideLayout.CellPadding), Ui.Dp((cellHeight - block) / 2));
                cell.Add(name);
                var span = Text.Label(clock.Span(program), PleyType.Caption, focused ? LiveViews.Alpha(PleyColors.OnAccent, 0.8f) : PleyColors.Subtle, width: textWidth);
                span.Position = new Position(Ui.Dp(offset + GuideLayout.CellPadding), Ui.Dp((cellHeight - block) / 2 + PleyType.Label.Line));
                cell.Add(span);
            }
            if (airing)
            {
                cell.Add(new View
                {
                    Position = new Position(0, Ui.Dp(cellHeight - GuideLayout.ProgressHeight)),
                    Size = new Size(Ui.Dp(cellWidth * program.ProgressAt(look.Now)), Ui.Dp(GuideLayout.ProgressHeight)),
                    BackgroundColor = focused ? PleyColors.OnAccent : PleyColors.Accent,
                });
            }
            row.Add(cell);
        }
        return row;
    }

    protected override void Dispose(DisposeTypes type)
    {
        if (Disposed) return;
        slide?.Stop();
        base.Dispose(type);
    }
}

/// <summary>
/// A channel's cell at the left of its guide row: its logo, number (a star when a favourite) and
/// name; the focused row's filled with Surface3. Under the whole row, its hairline.
/// </summary>
sealed class GuideChannelCell : View
{
    readonly View box;
    readonly TextLabel? name;

    public GuideChannelCell(LiveChannel? channel, string label, string origin, Images images, float rowWidthDp)
    {
        Channel = channel;
        Size = new Size(Ui.Dp(rowWidthDp), Ui.Dp(GuideLayout.RowHeight));
        Add(new View
        {
            Position = new Position(0, Ui.Dp(GuideLayout.RowHeight) - 1),
            Size = new Size(Ui.Dp(rowWidthDp), 1),
            BackgroundColor = LiveViews.Alpha(PleyColors.Surface4, 0.6f),
        });
        var inner = GuideLayout.RowHeight - 2 * GuideLayout.CellInsetY;
        box = new View
        {
            Position = new Position(0, Ui.Dp(GuideLayout.CellInsetY)),
            Size = new Size(Ui.Dp(channel == null ? GuideLayout.ChannelWidth - 8 : GuideLayout.ChannelWidth), Ui.Dp(inner)),
            CornerRadius = Ui.Dp(PleyShape.ControlRadius),
            BackgroundColor = channel == null ? PleyColors.Surface2 : Color.Transparent,
        };
        Add(box);
        if (channel == null) return;
        var logo = new ChannelLogoView(channel, origin, images, 56, 34);
        logo.Position = new Position(Ui.Dp(GuideLayout.ChannelInset), Ui.Dp((inner - 34) / 2));
        box.Add(logo);
        var x = GuideLayout.ChannelInset + 56 + 10;
        var textWidth = GuideLayout.ChannelWidth - x - 8;
        var top = (inner - PleyType.Caption.Line - PleyType.Label.Line) / 2;
        var number = LiveViews.Fitted(channel.Number ?? "", PleyType.Caption, PleyColors.Subtle, textWidth - 16);
        number.Position = new Position(Ui.Dp(x), Ui.Dp(top));
        box.Add(number);
        if (channel.Favorite)
        {
            var star = Icons.Icon("star", 12, PleyColors.Accent);
            star.Position = new Position(Ui.Dp(x) + number.SizeWidth + Ui.Dp(4), Ui.Dp(top + (PleyType.Caption.Line - 12) / 2));
            box.Add(star);
        }
        name = Text.Label(label, PleyType.Label, PleyColors.Muted, width: textWidth);
        name.Position = new Position(Ui.Dp(x), Ui.Dp(top + PleyType.Caption.Line));
        box.Add(name);
    }

    public LiveChannel? Channel { get; }

    public void Focus(bool on)
    {
        if (Disposed || name == null) return;
        box.BackgroundColor = on ? PleyColors.Surface3 : Color.Transparent;
        name.TextColor = on ? PleyColors.Fg : PleyColors.Muted;
    }
}
