using System;
using System.Collections.Generic;
using Pley.Tv.Core.Player;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Screens;

/// <summary>
/// The chapters as a row of cards over the scrubber, as YouTube's TV app has them: Up from the
/// scrubber opens it on the chapter playing, Left and Right go through the chapters, OK goes to
/// the one focused, Down or Back closes it. Each card has the chapter's picture (its own, else the
/// server's preview at its start), its name and when it starts.
/// </summary>
sealed class ChapterStrip : View
{
    public const float CardWidth = 176, PictureHeight = 99, Gap = 14;
    /// <summary>A card's height: the picture, 6 dp, then the name and the time.</summary>
    public static readonly float CardHeight = PictureHeight + 6 + PleyType.Label.Line + PleyType.Caption.Line;

    readonly Images images;
    readonly Func<long, TrickplayFrame?> previewAt;
    readonly Action<long> go;
    readonly Action close;
    readonly View strip = new View();
    readonly List<Card> cards = new List<Card>();
    /// <summary>Sheets fetched, by address: cards on the same sheet show at once.</summary>
    readonly Dictionary<string, string> sheets = new Dictionary<string, string>();
    IReadOnlyList<ChapterMark> chapters = Array.Empty<ChapterMark>();
    int index;
    Animation? slide;

    /// <param name="previewAt">The server's preview at a time (the player model's), null without.</param>
    /// <param name="go">Goes to a time: the chapter chosen's start.</param>
    /// <param name="close">The strip closes: focus back to the scrubber.</param>
    public ChapterStrip(Images images, Func<long, TrickplayFrame?> previewAt, Action<long> go, Action close)
    {
        this.images = images;
        this.previewAt = previewAt;
        this.go = go;
        this.close = close;
        Size = new Size(Ui.Dp(PleyTv.CanvasWidth), Ui.Dp(CardHeight + 12));
        ClippingMode = ClippingModeType.ClipChildren;
        strip.Position = new Position(0, Ui.Dp(6));
        Add(strip);
        Item = new FocusItem("chapters")
        {
            OwnKey = (key, down) =>
            {
                switch (key)
                {
                    case "Left":
                    case "Right":
                        if (down) Move(key == "Left" ? -1 : 1);
                        return true;
                    case "Return":
                    case "KP_Enter":
                        // On the way up, as OK selects elsewhere.
                        if (!down && index < chapters.Count) go(chapters[index].StartMs);
                        return true;
                    case "Down":
                        if (down) close();
                        return true;
                    case "Up":
                        return true;
                    default:
                        return false;
                }
            },
        };
        Hide();
    }

    public FocusItem Item { get; }

    public bool Open => Visibility;

    /// <summary>The chapter focused changed: its start, for the scrubber and the seek preview to show.</summary>
    public Action<long>? Focused { get; set; }

    /// <summary>Opens on <paramref name="at"/>'s chapter.</summary>
    public void Show(IReadOnlyList<ChapterMark> marks, long at)
    {
        if (!ReferenceEquals(marks, chapters))
        {
            chapters = marks;
            foreach (var card in cards) card.Dispose();
            cards.Clear();
            for (var i = 0; i < marks.Count; i++)
            {
                var card = new Card(marks[i], i);
                card.PositionX = Ui.Dp(PleyTv.Gutter + i * (CardWidth + Gap));
                strip.Add(card);
                cards.Add(card);
            }
        }
        index = Math.Max(0, PlayerText.ChapterAt(marks, at) ?? Nearest(marks, at));
        base.Show();
        Focus(animate: false);
    }

    /// <summary>The chapter before <paramref name="at"/> when it falls between chapters.</summary>
    static int Nearest(IReadOnlyList<ChapterMark> marks, long at)
    {
        var found = 0;
        for (var i = 0; i < marks.Count; i++)
            if (marks[i].StartMs <= at) found = i;
        return found;
    }

    void Move(int by)
    {
        var next = Math.Max(0, Math.Min(chapters.Count - 1, index + by));
        if (next == index) return;
        index = next;
        Focus(animate: true);
    }

    /// <summary>The focused card's look; the row slides so it stays in view, the first at the gutter.</summary>
    void Focus(bool animate)
    {
        for (var i = 0; i < cards.Count; i++) cards[i].Look(i == index);
        if (index < chapters.Count) Focused?.Invoke(chapters[index].StartMs);
        // Pictures for the cards near the focused one.
        for (var i = Math.Max(0, index - 2); i <= Math.Min(cards.Count - 1, index + 5); i++) Picture(cards[i], i);
        var room = PleyTv.CanvasWidth - PleyTv.Gutter - PleyTv.SafeHorizontal;
        var content = cards.Count * (CardWidth + Gap) - Gap;
        var x = content <= room ? 0 : -Math.Min(index * (CardWidth + Gap), content - room);
        slide?.Stop();
        if (!animate)
        {
            strip.PositionX = Ui.Dp(x);
            return;
        }
        slide = new Animation(PleyMotion.FocusMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.EaseOut) };
        slide.AnimateTo(strip, "PositionX", Ui.Dp(x));
        slide.Play();
    }

    void Picture(Card card, int i)
    {
        if (card.Asked) return;
        card.Asked = true;
        var chapter = chapters[i];
        // The chapter's own picture, else the server's preview a moment into it.
        var frame = chapter.ImageUrl == null ? previewAt(chapter.StartMs / Trickplay.StepMs * Trickplay.StepMs + Trickplay.StepMs) : null;
        var url = chapter.ImageUrl ?? frame?.SheetUrl;
        if (url == null) return;
        if (sheets.TryGetValue(url, out var known))
        {
            card.Show(known, frame);
            return;
        }
        images.Load(new[] { url }, frame != null ? frame.SheetWidth / Ui.Scale : CardWidth, file =>
        {
            if (file == null || Disposed) return;
            sheets[url] = file;
            card.Show(file, frame);
        });
    }

    protected override void Dispose(DisposeTypes type)
    {
        if (Disposed) return;
        slide?.Stop();
        base.Dispose(type);
    }

    /// <summary>A chapter: its picture on Surface2, its name, its start; focused, the accent ring and Fg text.</summary>
    sealed class Card : View
    {
        readonly ImageView picture;
        readonly View ring;
        readonly TextLabel name;
        readonly TextLabel start;

        public Card(ChapterMark chapter, int index)
        {
            Size = new Size(Ui.Dp(CardWidth), Ui.Dp(CardHeight));
            var well = new View
            {
                Size = new Size(Ui.Dp(CardWidth), Ui.Dp(PictureHeight)),
                CornerRadius = Ui.Dp(PleyShape.TileRadius),
                BackgroundColor = PleyColors.Surface2,
            };
            picture = new ImageView
            {
                Size = well.Size,
                CornerRadius = Ui.Dp(PleyShape.TileRadius),
                FittingMode = FittingModeType.ScaleToFill,
            };
            well.Add(picture);
            Add(well);
            var reach = Ui.Dp(PleyShape.ThumbRingOffset + PleyShape.ThumbRingStroke / 2);
            ring = new View
            {
                Size = new Size(Ui.Dp(CardWidth) + 2 * reach, Ui.Dp(PictureHeight) + 2 * reach),
                Position = new Position(-reach, -reach),
                BackgroundColor = Color.Transparent,
                BorderlineWidth = Ui.Dp(PleyShape.RingStroke),
                BorderlineColor = PleyColors.AccentBright,
                BorderlineOffset = -1f,
                CornerRadius = Ui.Dp(PleyShape.TileRadius) + reach,
            };
            ring.Hide();
            Add(ring);
            name = Text.Label(PlayerText.ChapterTitle(chapter, index), PleyType.Label, PleyColors.Muted, width: CardWidth);
            name.PositionY = Ui.Dp(PictureHeight + 6);
            Add(name);
            start = Text.Label(PlayerText.Time(chapter.StartMs), PleyType.Caption, PleyColors.Subtle, width: CardWidth);
            start.PositionY = Ui.Dp(PictureHeight + 6 + PleyType.Label.Line);
            Add(start);
        }

        public bool Asked { get; set; }

        public void Look(bool focused)
        {
            if (Disposed) return;
            if (focused) ring.Show();
            else ring.Hide();
            name.TextColor = focused ? PleyColors.Fg : PleyColors.Muted;
        }

        public void Show(string file, TrickplayFrame? crop)
        {
            if (Disposed) return;
            picture.ResourceUrl = file;
            picture.PixelArea = crop == null
                ? new RelativeVector4(0, 0, 1, 1)
                : new RelativeVector4(crop.Left / (float)crop.SheetWidth, crop.Top / (float)crop.SheetHeight, crop.Width / (float)crop.SheetWidth, crop.Height / (float)crop.SheetHeight);
        }
    }
}
