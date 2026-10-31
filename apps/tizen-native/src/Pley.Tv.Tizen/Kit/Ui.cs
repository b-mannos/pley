using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// The design's units: the Android TV app draws on a 960 × 540 dp canvas (PleyTv.CanvasWidth),
/// and this app's window is 1920 × 1080 pixels, so a dp or an sp is <see cref="Scale"/> pixels.
/// Screens are written in the Android app's dp, through <see cref="Dp"/>.
/// </summary>
static class Ui
{
    /// <summary>Pixels per dp: the window's width over the canvas's 960.</summary>
    public static float Scale { get; private set; } = 2f;

    public static void Measure(Window window) => Scale = window.Size.Width / 960f;

    public static float Dp(float dp) => dp * Scale;

    public static int DpInt(float dp) => (int)System.Math.Round(dp * Scale);
}

/// <summary>Pley's colours (the Android app's PleyColors, design spec §1.1).</summary>
static class PleyColors
{
    public static readonly Color Bg = Hex(0x0B0D10);
    /// <summary>The expanded rail, settings groups.</summary>
    public static readonly Color Surface = Hex(0x12151A);
    /// <summary>Tile wells, inputs, dialogs, panels, skeletons.</summary>
    public static readonly Color Surface2 = Hex(0x191D24);
    /// <summary>Unfocused buttons and chips, the current page in lists.</summary>
    public static readonly Color Surface3 = Hex(0x222731);
    /// <summary>Dividers, hairlines, the progress track.</summary>
    public static readonly Color Surface4 = Hex(0x2A303B);
    /// <summary>Decorative borders only.</summary>
    public static readonly Color Outline = Hex(0x3A4250);
    /// <summary>Edges of interactive things: input outlines, empty PIN dots, outlined pills.</summary>
    public static readonly Color Control = Hex(0x6C7584);
    /// <summary>Primary text and icons. Text is never pure white.</summary>
    public static readonly Color Fg = Hex(0xE9EBEF);
    public static readonly Color Muted = Hex(0xA1A9B6);
    /// <summary>Tertiary text: dates, separators.</summary>
    public static readonly Color Subtle = Hex(0x8B95A5);
    public static readonly Color Disabled = Hex(0x6C7584);
    /// <summary>TV focus fills; progress; checks.</summary>
    public static readonly Color Accent = Hex(0x38BDF8);
    /// <summary>The TV focus ring.</summary>
    public static readonly Color AccentBright = Hex(0x7DD3FC);
    /// <summary>Content on Accent and AccentBright.</summary>
    public static readonly Color OnAccent = Hex(0x03131F);
    public static readonly Color BrandLight = Hex(0xBAE6FD);
    public static readonly Color BrandDeep = Hex(0x0284C7);
    public static readonly Color BrandFar = Hex(0x0C4A6E);
    public static readonly Color BrandTile = Hex(0x0A1622);
    public static readonly Color Success = Hex(0x3FB27F);
    public static readonly Color Danger = Hex(0xE5484D);
    public static readonly Color DangerText = Hex(0xFF6B70);
    /// <summary>Badges, the watched tick and round buttons on art: Bg at 80 %.</summary>
    public static readonly Color OnArt = Hex(0x0B0D10, 0.8f);
    /// <summary>The progress track over art: black at 60 %.</summary>
    public static readonly Color ProgressTrackOnArt = new Color(0, 0, 0, 0.6f);
    /// <summary>The hairline inside a tile's well: white at 6 %.</summary>
    public static readonly Color TileHairline = new Color(1, 1, 1, 0.06f);
    /// <summary>Behind dialogs, sheets and panels: Bg at 60 %.</summary>
    public static readonly Color Scrim = Hex(0x0B0D10, 0.6f);

    static Color Hex(int rgb, float alpha = 1f) =>
        new Color(((rgb >> 16) & 0xff) / 255f, ((rgb >> 8) & 0xff) / 255f, (rgb & 0xff) / 255f, alpha);
}

/// <summary>A text style: size and line height in sp, and weight (the Android app's PleyType).</summary>
sealed record TextStyle(float Size, float Line, string Weight);

/// <summary>Pley's TV type (PleyType.Tv, design spec §1.2): weights 400, 500 and 700 only.</summary>
static class PleyType
{
    public const string Regular = "normal";
    public const string Medium = "semiBold";
    public const string Bold = "bold";

    /// <summary>A title without logo art; 2 lines at most.</summary>
    public static readonly TextStyle HeroTitle = new TextStyle(38, 44, Bold);
    /// <summary>Library, Search, Settings, Who's watching?</summary>
    public static readonly TextStyle PageTitle = new TextStyle(28, 34, Bold);
    /// <summary>Row headers.</summary>
    public static readonly TextStyle Section = new TextStyle(18, 24, Bold);
    /// <summary>Episode titles in info blocks; panel and dialog titles; placeholder titles on tiles.</summary>
    public static readonly TextStyle Title = new TextStyle(16, 22, Bold);
    /// <summary>Summaries on the hero and the item page.</summary>
    public static readonly TextStyle BodyLarge = new TextStyle(15, 22, Regular);
    /// <summary>Lists, settings rows, instructions, 2-line summaries.</summary>
    public static readonly TextStyle Body = new TextStyle(14, 20, Regular);
    /// <summary>Buttons, the metadata line, tabs, chips.</summary>
    public static readonly TextStyle Label = new TextStyle(14, 18, Medium);
    /// <summary>Tile captions, scrubber times.</summary>
    public static readonly TextStyle Caption = new TextStyle(13, 17, Medium);
    /// <summary>Quality badges, as written ("1080p").</summary>
    public static readonly TextStyle Badge = new TextStyle(11, 13, Bold);
    /// <summary>The link code.</summary>
    public static readonly TextStyle Code = new TextStyle(48, 56, Bold);
    /// <summary>PIN and address pad keys.</summary>
    public static readonly TextStyle PinDigit = new TextStyle(28, 28, Medium);
}

/// <summary>The TV layout's fixed lines on the 960 × 540 canvas (PleyTv, TvSafeArea, PleySpace).</summary>
static class PleyTv
{
    public const float CanvasWidth = 960, CanvasHeight = 540;
    /// <summary>What a TV keeps clear around the picture (overscan): 48 at the sides, 27 above and below.</summary>
    public const float SafeHorizontal = 48, SafeVertical = 27;
    /// <summary>The collapsed rail: 8 + a 56 dp item + 8.</summary>
    public const float RailCollapsed = 72;
    /// <summary>The expanded rail: 8 + a 256 dp item + 8.</summary>
    public const float RailExpanded = 272;
    /// <summary>Every page's left edge: the rail and 24. Rows keep their focused tile here.</summary>
    public const float Gutter = 96;
    /// <summary>The right edge of text over backdrops.</summary>
    public const float TextColumnEnd = 560;
    /// <summary>Where the page puts a focused row's header, under the info block.</summary>
    public const float FocusedRowHeaderY = 172;
    public const float RowHeaderHeight = 24;
    /// <summary>From a row header to its tiles: room for the focused tile's growth and ring.</summary>
    public const float HeaderToTiles = 14;
    public const float TilesToCaption = 14;
    /// <summary>A caption's two lines of Caption.</summary>
    public const float CaptionHeight = 34;
    /// <summary>Between one row's caption and the next row's header.</summary>
    public const float RowGap = 28;
    /// <summary>Between posters, squares and people.</summary>
    public const float TileGap = 14;
    /// <summary>Between landscape and episode tiles, which grow more when focused.</summary>
    public const float WideTileGap = 18;
    public const float ButtonGap = 10;
    public const float ButtonHeight = 40;
    public const float ListRowHeight = 44;
    public const float IconSize = 20;
    /// <summary>The rows' fade under the collapsed rail: Bg to this x…</summary>
    public const float RailFadeSolidUntil = 56;
    /// <summary>…and clear from this x.</summary>
    public const float RailFadeClearFrom = 72;
}

/// <summary>Corner radii (PleyShape) and the focus look (PleyFocus).</summary>
static class PleyShape
{
    public const float BadgeRadius = 2, TileRadius = 4, FieldRadius = 4, ControlRadius = 6, CardRadius = 6, DialogRadius = 8;
    /// <summary>The focus ring: a 3 dp AccentBright stroke centred 4.5 dp outside the tile.</summary>
    public const float RingStroke = 3, RingOffset = 4.5f;
    public const float TileScale = 1.08f, TilePressedScale = 1.04f, RoundScale = 1.08f, AvatarScale = 1.1f, PillScale = 1.05f;
}

/// <summary>How Pley moves (PleyMotion, design spec §5.1): short, eased, never bouncing.</summary>
static class PleyMotion
{
    public const int FocusMs = 150, UnfocusMs = 120, PressMs = 100, RevealMs = 150, BackdropMs = 300, RestMs = 400, OverlayMs = 200, ScreenMs = 220, ImageFadeMs = 150;
}

/// <summary>Text the way the Android app sets it: the style's size and weight, one line unless asked, ellipsized.</summary>
static class Text
{
    public static TextLabel Label(string text, TextStyle style, Color color, int maxLines = 1, float width = 0)
    {
        var label = new TextLabel(text)
        {
            PixelSize = Ui.Dp(style.Size),
            TextColor = color,
            MultiLine = maxLines > 1,
            Ellipsis = true,
            FontStyle = Weight(style.Weight),
            VerticalAlignment = VerticalAlignment.Center,
        };
        if (width > 0) label.Size = new Size(Ui.Dp(width), maxLines > 1 ? Ui.Dp(style.Line * maxLines) : Ui.Dp(style.Line));
        return label;
    }

    public static PropertyMap Weight(string weight)
    {
        var map = new PropertyMap();
        map.Add("weight", new PropertyValue(weight));
        return map;
    }
}
