using Tizen.NUI;

namespace Pley.Tv.Tizen;

/// <summary>The app's colours: the web TV app's (packages/tv/src/ui/theme.css).</summary>
static class Theme
{
    public static readonly Color Background = Hex(0x0b0d10);
    public static readonly Color Surface = Hex(0x12151a);
    public static readonly Color Foreground = Hex(0xe9ebef);
    public static readonly Color Muted = Hex(0xa1a9b6);
    public static readonly Color Accent = Hex(0x38bdf8);

    static Color Hex(int rgb) => new Color(((rgb >> 16) & 0xff) / 255f, ((rgb >> 8) & 0xff) / 255f, (rgb & 0xff) / 255f, 1f);
}
