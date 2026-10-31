using System;
using Pley.Tv.Core.Ui;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// A tile on TV (the Android app's TvTile and ArtworkTile, design spec §2.3, D-pad rule 8): the
/// well with its picture, or the title on a placeholder until the picture comes, and its overlays;
/// a caption under it. Focused, the well grows to 1.08 and gets the accent ring around it, and the
/// caption turns bold. While a key repeats it snaps to its focus look (rule 5).
/// </summary>
sealed class TileView : View
{
    readonly TileKind kind;
    readonly View frame;
    readonly View ring;
    readonly ImageView picture;
    readonly TextLabel? placeholder;
    readonly TextLabel title;
    readonly TextLabel? subtitle;
    Animation? focusAnimation;
    bool pictureAsked;

    public TileView(TileModel model, TileKind kind, FocusItem focus, KeyRepeatGate gate, Images images)
    {
        this.kind = kind;
        Model = model;
        Focus = focus;
        var w = Ui.Dp(kind.Width);
        var h = Ui.Dp(kind.Height);
        var radius = kind.Round ? w / 2 : Ui.Dp(PleyShape.TileRadius);
        Size = new Size(w, h + Ui.Dp(PleyTv.TilesToCaption + PleyTv.CaptionHeight));

        // The well and its ring scale together, about the well's centre.
        frame = new View
        {
            Size = new Size(w, h),
            PivotPoint = global::Tizen.NUI.PivotPoint.Center,
            PositionUsesPivotPoint = false,
        };
        // The ring: a 3 dp stroke centred 4.5 dp outside the well, so 3 dp clear of it.
        var reach = Ui.Dp(PleyShape.RingOffset + PleyShape.RingStroke / 2);
        ring = new View
        {
            Size = new Size(w + 2 * reach, h + 2 * reach),
            Position = new Position(-reach, -reach),
            BackgroundColor = Color.Transparent,
            BorderlineWidth = Ui.Dp(PleyShape.RingStroke),
            BorderlineColor = PleyColors.AccentBright,
            BorderlineOffset = -1f,
            CornerRadius = kind.Round ? (w + 2 * reach) / 2 : radius + reach,
            Opacity = 0,
        };
        frame.Add(ring);
        var well = new View
        {
            Size = new Size(w, h),
            BackgroundColor = PleyColors.Surface2,
            CornerRadius = radius,
            ClippingMode = ClippingModeType.ClipChildren,
        };
        frame.Add(well);
        if (!kind.Round)
        {
            placeholder = Text.Label(model.Title, PleyType.Title, PleyColors.Muted, maxLines: 4);
            placeholder.Size = new Size(w - Ui.Dp(16), h - Ui.Dp(16));
            placeholder.Position = new Position(Ui.Dp(8), Ui.Dp(8));
            placeholder.HorizontalAlignment = HorizontalAlignment.Center;
            placeholder.VerticalAlignment = VerticalAlignment.Center;
            well.Add(placeholder);
        }
        picture = new ImageView
        {
            Size = new Size(w, h),
            CornerRadius = radius,
            // Decoded at the size it's drawn, not the file's.
            DesiredWidth = (int)w,
            DesiredHeight = (int)h,
            FittingMode = FittingModeType.ScaleToFill,
        };
        well.Add(picture);
        Overlays(well, model, w, h);
        Add(frame);

        var captionTop = h + Ui.Dp(PleyTv.TilesToCaption);
        title = Text.Label(model.Title, PleyType.Caption, PleyColors.Fg, width: kind.Width);
        title.Position = new Position(0, captionTop);
        if (kind.Round) title.HorizontalAlignment = HorizontalAlignment.Center;
        Add(title);
        if (model.Subtitle.Length > 0)
        {
            subtitle = Text.Label(model.Subtitle, PleyType.Caption, PleyColors.Muted, width: kind.Width);
            subtitle.Position = new Position(0, captionTop + Ui.Dp(PleyType.Caption.Line));
            if (kind.Round) subtitle.HorizontalAlignment = HorizontalAlignment.Center;
            Add(subtitle);
        }

        this.images = images;
        this.gate = gate;
        focus.FocusChanged += OnFocus;
    }

    readonly Images images;
    readonly KeyRepeatGate gate;

    public TileModel Model { get; }
    public FocusItem Focus { get; }

    /// <summary>Asks for the picture once the tile is near the screen; it fades in over the placeholder.</summary>
    public void LoadPicture()
    {
        if (pictureAsked || Model.Images.Count == 0) return;
        pictureAsked = true;
        picture.Opacity = 0;
        images.Load(Model.Images, kind.Width, file =>
        {
            if (file == null || Disposed) return;
            picture.ResourceUrl = file;
            var fade = new Animation(PleyMotion.ImageFadeMs);
            fade.AnimateTo(picture, "Opacity", 1f);
            fade.Play();
        });
    }

    void OnFocus(bool on)
    {
        // Drawn over its neighbours and their captions while focused.
        if (on) RaiseToTop();
        title.FontStyle = Text.Weight(on ? PleyType.Bold : PleyType.Caption.Weight);
        var scale = on ? (kind.Round ? PleyShape.AvatarScale : PleyShape.TileScale) : 1f;
        focusAnimation?.Stop();
        if (gate.Holding)
        {
            // Held keys snap (rule 5).
            frame.Scale = new Vector3(scale, scale, 1);
            ring.Opacity = on ? 1 : 0;
            return;
        }
        focusAnimation = new Animation(on ? PleyMotion.FocusMs : PleyMotion.UnfocusMs)
        {
            DefaultAlphaFunction = new AlphaFunction(on ? AlphaFunction.BuiltinFunctions.EaseOut : AlphaFunction.BuiltinFunctions.EaseIn),
        };
        focusAnimation.AnimateTo(frame, "Scale", new Vector3(scale, scale, 1));
        focusAnimation.AnimateTo(ring, "Opacity", on ? 1f : 0f);
        focusAnimation.Play();
    }

    /// <summary>The progress bar along the bottom, the watched tick or unwatched count top right, badges bottom left.</summary>
    static void Overlays(View well, TileModel model, float w, float h)
    {
        var inset = Ui.Dp(6);
        var bar = model.Progress > 0 ? Ui.Dp(4) : 0;
        if (bar > 0)
        {
            well.Add(new View { Size = new Size(w, bar), Position = new Position(0, h - bar), BackgroundColor = PleyColors.ProgressTrackOnArt });
            well.Add(new View { Size = new Size(w * Math.Min(1f, model.Progress), bar), Position = new Position(0, h - bar), BackgroundColor = PleyColors.Accent });
        }
        if (model.Unwatched > 0)
        {
            var count = Text.Label(model.Unwatched.ToString(System.Globalization.CultureInfo.InvariantCulture), PleyType.Badge, PleyColors.Fg);
            var pill = new View
            {
                Size = new Size(Math.Max(Ui.Dp(18), Ui.Dp(8) + count.NaturalSize2D.Width), Ui.Dp(18)),
                BackgroundColor = PleyColors.OnArt,
                CornerRadius = Ui.Dp(9),
            };
            pill.Position = new Position(w - inset - pill.SizeWidth, inset);
            count.Size = new Size(pill.SizeWidth, pill.SizeHeight);
            count.HorizontalAlignment = HorizontalAlignment.Center;
            pill.Add(count);
            well.Add(pill);
        }
        else if (model.Watched)
        {
            var tick = new View
            {
                Size = new Size(Ui.Dp(20), Ui.Dp(20)),
                Position = new Position(w - inset - Ui.Dp(20), inset),
                BackgroundColor = PleyColors.OnArt,
                CornerRadius = Ui.Dp(10),
            };
            var glyph = Text.Label("✓", new TextStyle(12, 20, PleyType.Bold), PleyColors.Fg);
            glyph.Size = new Size(Ui.Dp(20), Ui.Dp(20));
            glyph.HorizontalAlignment = HorizontalAlignment.Center;
            tick.Add(glyph);
            well.Add(tick);
        }
        var x = inset;
        foreach (var badge in model.Badges)
        {
            var text = Text.Label(badge, PleyType.Badge, PleyColors.Fg);
            var box = new View
            {
                Size = new Size(text.NaturalSize2D.Width + Ui.Dp(12), Ui.Dp(18)),
                Position = new Position(x, h - bar - inset - Ui.Dp(18)),
                BackgroundColor = PleyColors.OnArt,
                CornerRadius = Ui.Dp(PleyShape.BadgeRadius),
            };
            text.Size = new Size(box.SizeWidth, box.SizeHeight);
            text.HorizontalAlignment = HorizontalAlignment.Center;
            box.Add(text);
            well.Add(box);
            x += box.SizeWidth + Ui.Dp(4);
        }
    }
}
