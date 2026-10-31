using System;
using System.Collections.Generic;
using Pley.Tv.Core.Ui;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// A side panel (design spec §2.7; the Android app's TvSidePanel and TvPanelRow): 360 dp from the
/// right edge, full height, Surface2 with 16 dp left corners, over the scrim. Its own sheet keeps
/// focus inside it, starting on the row it is given (the current choice); Back closes it, and the
/// page's focus is where it was. Rows are 44 tall, Body text with an optional icon and a check;
/// focused, an accent pill, never scaled. Rows past the screen scroll into view.
/// </summary>
sealed class SidePanelSheet : Sheet
{
    const float Width = 360, Left = 24, RowHeight = 44, HeadingHeight = 36;

    readonly Shell shell;
    readonly View list = new View();
    readonly List<(View View, float Top)> placed = new List<(View, float)>();
    float scroll;

    /// <param name="choose">What a row does; true keeps the panel open (a track picked), false closes it.</param>
    public SidePanelSheet(Shell shell, SidePanel panel, Func<PanelRow, bool> choose) : base("panel")
    {
        this.shell = shell;
        View.BackgroundColor = PleyColors.Scrim;
        var card = new View
        {
            Size = new Size(Ui.Dp(Width), Ui.Dp(PleyTv.CanvasHeight)),
            Position = new Position(Ui.Dp(PleyTv.CanvasWidth - Width), 0),
            BackgroundColor = PleyColors.Surface2,
            CornerRadius = new Vector4(Ui.Dp(PleyShape.DialogRadius * 2), 0, 0, Ui.Dp(PleyShape.DialogRadius * 2)),
            ClippingMode = ClippingModeType.ClipChildren,
        };
        var inner = Width - Left - PleyTv.SafeHorizontal;
        var title = Text.Label(panel.Title, PleyType.Title, PleyColors.Fg, width: inner);
        title.Position = new Position(Ui.Dp(Left), Ui.Dp(PleyTv.SafeVertical));
        card.Add(title);
        var listTop = PleyTv.SafeVertical + PleyType.Title.Line + 12;
        var viewport = new View
        {
            Position = new Position(Ui.Dp(Left), Ui.Dp(listTop)),
            Size = new Size(Ui.Dp(inner), Ui.Dp(PleyTv.CanvasHeight - listTop - PleyTv.SafeVertical)),
            ClippingMode = ClippingModeType.ClipChildren,
        };
        viewport.Add(list);
        card.Add(viewport);
        View.Add(card);

        var group = Root;
        FocusItem? first = null;
        float y = 0;
        for (var i = 0; i < panel.Rows.Count; i++)
        {
            var row = panel.Rows[i];
            if (row.Kind == PanelRowKind.Heading)
            {
                var heading = Text.Label(row.Label, PleyType.Caption, PleyColors.Muted, width: inner);
                heading.Position = new Position(Ui.Dp(16), Ui.Dp(y + HeadingHeight - PleyType.Caption.Line - 6));
                list.Add(heading);
                y += HeadingHeight;
                continue;
            }
            var view = new PanelRowView(row, inner, () =>
            {
                if (!choose(row)) Close();
            });
            view.PositionY = Ui.Dp(y);
            list.Add(view);
            group.Add(view.Item);
            var top = y;
            view.Item.FocusChanged += on =>
            {
                if (on) Reveal(top, viewport.SizeHeight / Ui.Scale);
            };
            if (i == panel.Focus || first == null && i > panel.Focus) first ??= view.Item;
            y += RowHeight + 4;
        }
        first ??= group.Children.Count > 0 ? group.Children[0] as FocusItem : null;
        if (first != null) Tree.Focus(first);
    }

    public void Show() => shell.Push(this);

    public void Close() => shell.Pop(this);

    /// <summary>Keeps the focused row inside the panel's room.</summary>
    void Reveal(float top, float room)
    {
        if (top < scroll) scroll = top;
        else if (top + RowHeight > scroll + room) scroll = top + RowHeight - room;
        list.PositionY = -Ui.Dp(scroll);
    }

    /// <summary>One row: its label in Body, an optional 20 dp icon, an accent check when chosen.</summary>
    sealed class PanelRowView : View
    {
        readonly TextLabel label;
        readonly ImageView? icon;
        readonly ImageView? check;

        public PanelRowView(PanelRow row, float width, Action choose)
        {
            Size = new Size(Ui.Dp(width), Ui.Dp(RowHeight));
            CornerRadius = Ui.Dp(RowHeight / 2);
            Item = new FocusItem(row.Label) { Selected = choose };
            float x = 16;
            if (row.Icon != null)
            {
                icon = Icons.Icon(row.Icon, PleyTv.IconSize, PleyColors.Fg);
                icon.Position = new Position(Ui.Dp(x), Ui.Dp((RowHeight - PleyTv.IconSize) / 2));
                Add(icon);
                x += PleyTv.IconSize + 12;
            }
            var room = width - x - 16 - (row.Checked ? PleyTv.IconSize + 12 : 0);
            label = Text.Label(row.Label, PleyType.Body, PleyColors.Fg, width: room);
            label.Position = new Position(Ui.Dp(x), Ui.Dp((RowHeight - PleyType.Body.Line) / 2));
            Add(label);
            if (row.Checked)
            {
                check = Icons.Icon("check", PleyTv.IconSize, PleyColors.Accent);
                check.Position = new Position(Ui.Dp(width - 16 - PleyTv.IconSize), Ui.Dp((RowHeight - PleyTv.IconSize) / 2));
                Add(check);
            }
            Item.FocusChanged += Look;
        }

        public FocusItem Item { get; }

        void Look(bool focused)
        {
            if (Disposed) return;
            BackgroundColor = focused ? PleyColors.Accent : Color.Transparent;
            var ink = focused ? PleyColors.OnAccent : PleyColors.Fg;
            label.TextColor = ink;
            if (icon != null) icon.Color = ink;
            if (check != null) check.Color = focused ? PleyColors.OnAccent : PleyColors.Accent;
        }
    }
}
