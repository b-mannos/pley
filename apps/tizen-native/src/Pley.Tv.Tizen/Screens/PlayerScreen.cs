using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Threading;
using Pley.Tv.Core;
using Pley.Tv.Core.Player;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;
using Timer = Tizen.NUI.Timer;

namespace Pley.Tv.Tizen.Screens;

/// <summary>
/// The player (design spec §2.7; the Android app's TvPlayerScreen) over the Core's
/// <see cref="PlayerModel"/>: the server's session on the TV's own player, on the video plane
/// under the window. Hidden controls: OK plays and pauses, Left and Right seek
/// (<see cref="Seeker"/>), Up shows the controls on the scrubber, Down on Play/Pause. Shown: the
/// title and clock at the top; the times, scrubber and Back 10 · Play/Pause · Forward 10 at the
/// bottom, over a black scrim; they go 5 s after the last key while it plays. Back hides them,
/// then leaves. The model reports where it is to the server, through the app's reporter, so the
/// last report goes out after the screen closed.
/// </summary>
sealed class PlayerScreen : Sheet
{
    const float TimesTop = 399, TrackCentre = 425, RowTop = 441, TrackHeight = 4, Thumb = 10, ThumbFocused = 12;
    const float Button = 36, PlayButton = 44, TransportIcon = 24, PlayIcon = 26, TransportGap = 16;
    /// <summary>The spinner shows once playback has waited this long (the Android app's 500 ms).</summary>
    const long SpinnerAfterMs = 500;
    /// <summary>The same media key from NUI and from the media key service this soon after counts once.</summary>
    const long MediaTwiceMs = 250;

    readonly Shell shell;
    readonly KeyRepeatGate gate;
    readonly Action closed;
    readonly SynchronizationContext? ui = SynchronizationContext.Current;
    readonly PlayerModel model;
    readonly Seeker seeker;
    readonly IDisposable keys;
    readonly List<IDisposable> watches = new List<IDisposable>();

    readonly View black;
    readonly View bars;
    readonly View spinner;
    readonly Animation spin;
    readonly TextLabel reconnecting;
    readonly View controls;
    readonly TextLabel title;
    readonly TextLabel line;
    readonly TextLabel clock;
    readonly TextLabel ends;
    readonly TextLabel elapsed;
    readonly TextLabel remaining;
    readonly TextLabel badge;
    readonly View track;
    readonly View played;
    readonly View thumb;
    readonly View thumbRing;
    readonly FocusItem scrubber;
    readonly PlayerButton playPause;
    readonly Timer tick;

    View? error;
    FocusGroup? errorButtons;
    PlayerPhase? errorShown;
    bool shown;
    bool gone;
    bool? wants;
    bool covered = true;
    /// <summary>After an error, black stays until the next session's first picture.</summary>
    bool awaitingFrame;
    long? lastFirstFrame;
    long waitingSince = -1;
    Animation? fade;
    Animation? coverFade;
    long hideAt;

    /// <param name="reporter">The app's reports queue: the last "stopped" goes out after the screen closed.</param>
    /// <param name="closed">The screen went (Home asks again for the resume points).</param>
    public PlayerScreen(Shell shell, AppState state, PlayRequest request, IReporter reporter, KeyRepeatGate gate, Action closed) : base("player")
    {
        Covers = true;
        this.shell = shell;
        this.gate = gate;
        this.closed = closed;
        var source = new AppPlayerSource(state, () => Caps(state));
        model = new PlayerModel(request, source, Engine, new ContextScheduler(), reporter);
        seeker = new Seeker(model.LivePositionMs, () => model.Clock.Value.DurationMs, SeekTo);

        // Black until the first picture: the video plane under it shows nothing of the page meanwhile.
        black = new View { Size = Canvas(), BackgroundColor = Color.Black };
        View.Add(black);
        bars = new View { Size = Canvas() };
        View.Add(bars);

        controls = new View { Size = Canvas(), Opacity = 0 };
        controls.Add(Scrim());
        title = Text.Label("", PleyType.Title, PleyColors.Fg, width: PleyTv.CanvasWidth - PleyTv.SafeHorizontal - 300);
        title.Position = new Position(Ui.Dp(PleyTv.SafeHorizontal), Ui.Dp(PleyTv.SafeVertical));
        controls.Add(title);
        line = Text.Label("", PleyType.Label, PleyColors.Muted, width: PleyTv.CanvasWidth - PleyTv.SafeHorizontal - 300);
        line.Position = new Position(Ui.Dp(PleyTv.SafeHorizontal), Ui.Dp(PleyTv.SafeVertical + PleyType.Title.Line + 4));
        controls.Add(line);
        clock = RightAligned(PleyType.Label, PleyColors.Muted, PleyTv.SafeVertical);
        controls.Add(clock);
        ends = RightAligned(PleyType.Label, PleyColors.Muted, PleyTv.SafeVertical + PleyType.Label.Line + 2);
        controls.Add(ends);

        var width = PleyTv.CanvasWidth - 2 * PleyTv.SafeHorizontal;
        elapsed = Text.Label("", PleyType.Caption, PleyColors.Fg, width: 200);
        elapsed.Position = new Position(Ui.Dp(PleyTv.SafeHorizontal), Ui.Dp(TimesTop));
        controls.Add(elapsed);
        remaining = Text.Label("", PleyType.Caption, PleyColors.Fg, width: 200);
        remaining.HorizontalAlignment = HorizontalAlignment.End;
        remaining.Position = new Position(Ui.Dp(PleyTv.CanvasWidth - PleyTv.SafeHorizontal - 200), Ui.Dp(TimesTop));
        controls.Add(remaining);
        badge = Text.Label("", PleyType.Label, PleyColors.Fg, width: 120);
        badge.HorizontalAlignment = HorizontalAlignment.Center;
        badge.PositionY = Ui.Dp(TimesTop - PleyType.Label.Line - 6);
        controls.Add(badge);

        track = new View
        {
            Position = new Position(Ui.Dp(PleyTv.SafeHorizontal), Ui.Dp(TrackCentre - TrackHeight / 2)),
            Size = new Size(Ui.Dp(width), Ui.Dp(TrackHeight)),
            CornerRadius = Ui.Dp(TrackHeight / 2),
            BackgroundColor = new Color(PleyColors.Fg.R, PleyColors.Fg.G, PleyColors.Fg.B, 0.25f),
        };
        played = new View { Size = new Size(0, Ui.Dp(TrackHeight)), CornerRadius = Ui.Dp(TrackHeight / 2), BackgroundColor = PleyColors.Accent };
        track.Add(played);
        controls.Add(track);
        var ring = ThumbFocused + 2 * PleyShape.ThumbRingOffset + 2 * PleyShape.ThumbRingStroke;
        thumbRing = new View
        {
            Size = new Size(Ui.Dp(ring), Ui.Dp(ring)),
            CornerRadius = Ui.Dp(ring / 2),
            BorderlineWidth = Ui.Dp(PleyShape.ThumbRingStroke),
            BorderlineColor = PleyColors.AccentBright,
            BorderlineOffset = -1,
            BackgroundColor = Color.Transparent,
            Opacity = 0,
        };
        controls.Add(thumbRing);
        thumb = new View { Size = new Size(Ui.Dp(Thumb), Ui.Dp(Thumb)), CornerRadius = Ui.Dp(Thumb / 2), BackgroundColor = PleyColors.Fg };
        controls.Add(thumb);

        // Back 10 · Play/Pause · Forward 10, centred; the smaller ones' centres in line with Play/Pause's.
        var back10 = new PlayerButton(PlayerText.Back10, "rotate-ccw", Button, TransportIcon, () => Jump(-1));
        playPause = new PlayerButton(PlayerText.Pause, "pause", PlayButton, PlayIcon, PlayPause);
        var forward10 = new PlayerButton(PlayerText.Forward10, "rotate-cw", Button, TransportIcon, () => Jump(1));
        var rowWidth = Button * 2 + PlayButton + TransportGap * 2;
        var x = (PleyTv.CanvasWidth - rowWidth) / 2;
        back10.Position = new Position(Ui.Dp(x), Ui.Dp(RowTop + (PlayButton - Button) / 2));
        playPause.Position = new Position(Ui.Dp(x + Button + TransportGap), Ui.Dp(RowTop));
        forward10.Position = new Position(Ui.Dp(x + Button + TransportGap + PlayButton + TransportGap), Ui.Dp(RowTop + (PlayButton - Button) / 2));
        controls.Add(back10);
        controls.Add(playPause);
        controls.Add(forward10);
        View.Add(controls);

        spinner = Spinner();
        spinner.Position = new Position(Ui.Dp(PleyTv.CanvasWidth / 2 - 20), Ui.Dp(PleyTv.CanvasHeight / 2 - 20));
        spinner.Hide();
        View.Add(spinner);
        spin = new Animation(1000) { Looping = true };
        spin.AnimateBy(spinner, "Orientation", new Rotation(new Radian(new Degree(360)), PositionAxis.Z));
        // "Reconnecting…" 16 dp under the ring, on the seek badge's backing: the picture behind it can be anything.
        reconnecting = Text.Label(PlayerText.Reconnecting, PleyType.Label, PleyColors.Fg);
        reconnecting.HorizontalAlignment = HorizontalAlignment.Center;
        var reconnectingWidth = reconnecting.NaturalSize.Width / Ui.Scale + 24;
        reconnecting.Size = new Size(Ui.Dp(reconnectingWidth), Ui.Dp(PleyType.Label.Line + 12));
        reconnecting.Position = new Position(Ui.Dp((PleyTv.CanvasWidth - reconnectingWidth) / 2), Ui.Dp(PleyTv.CanvasHeight / 2 + 20 + 16));
        reconnecting.BackgroundColor = new Color(PleyColors.Bg.R, PleyColors.Bg.G, PleyColors.Bg.B, 0.6f);
        reconnecting.Hide();
        View.Add(reconnecting);

        // Focus: the scrubber over the row; Left and Right on the scrubber seek, Down goes to Play/Pause.
        scrubber = new FocusItem("scrubber")
        {
            Selected = Ok,
            OwnKey = (key, down) =>
            {
                switch (key)
                {
                    case "Left":
                    case "Right":
                        if (down && !Busy) Step(key == "Left" ? -1 : 1);
                        return true;
                    case "Down":
                        if (down) Tree.Focus(playPause.Item);
                        return true;
                    default:
                        return false;
                }
            },
        };
        scrubber.FocusChanged += on =>
        {
            if (Disposed) return;
            thumbRing.Opacity = on ? 1 : 0;
            thumb.Size = new Size(Ui.Dp(on ? ThumbFocused : Thumb), Ui.Dp(on ? ThumbFocused : Thumb));
            thumb.CornerRadius = Ui.Dp((on ? ThumbFocused : Thumb) / 2);
            Layout();
        };
        Root.Add(scrubber);
        var row = new FocusGroup(Axis.Row, "transport");
        row.Add(back10.Item);
        row.Add(playPause.Item);
        row.Add(forward10.Item);
        row.RememberChild(1);
        Root.Add(row);

        keys = shell.Input.Push(OnKey);
        // The remote's media keys may come through the media key service rather than the window.
        try
        {
            global::Tizen.System.MediaKey.Event += OnMediaKey;
        }
        catch (Exception)
        {
            // Not on this TV: the window's keys only.
        }
        gate.HoldEnded += Released;
        shell.Shown += AppShown;
        Back = OnBack;
        Gone += Ended;

        tick = new Timer(250);
        tick.Tick += (_, _) =>
        {
            Tick();
            return !gone;
        };
        tick.Start();
        watches.Add(model.State.Watch(Render));
    }

    bool Disposed => gone;

    /// <summary>Starting or between sessions: Left, Right and OK wait, as on Android.</summary>
    bool Busy => model.State.Value.Phase.Kind == PhaseKind.Preparing;

    static Size Canvas() => new Size(Ui.Dp(PleyTv.CanvasWidth), Ui.Dp(PleyTv.CanvasHeight));

    static TextLabel RightAligned(TextStyle style, Color color, float top)
    {
        var label = Text.Label("", style, color, width: 240);
        label.HorizontalAlignment = HorizontalAlignment.End;
        label.Position = new Position(Ui.Dp(PleyTv.CanvasWidth - PleyTv.SafeHorizontal - 240), Ui.Dp(top));
        return label;
    }

    /// <summary>What the TV says it plays, with Settings' passthrough choices; null when it can't say.</summary>
    static IReadOnlyDictionary<string, object?>? Caps(AppState state)
    {
        try
        {
            return TizenCaps.Of(Platform.Facts(), state.Settings.Value.Passthrough);
        }
        catch (Exception)
        {
            // The server decides alone, as for a client that sends no report.
            return null;
        }
    }

    /// <summary>The TV's player for the video: on the video plane, its picture's size telling where black bars go.</summary>
    IVideoEngine Engine()
    {
        var engine = new TizenMediaEngine(video: true);
        engine.VideoSize += Bars;
        return engine;
    }

    /// <summary>Shows the player and starts: the item's details, a session, its stream.</summary>
    public void Show()
    {
        shell.Push(this);
        model.Start();
    }

    // ---- what the playback says

    void Render(PlayerState s)
    {
        if (gone) return;
        title.Text = s.Title;
        line.Text = s.Subtitle ?? "";
        var wantsToPlay = s.Phase.WantsToPlay;
        if (wants != wantsToPlay)
        {
            wants = wantsToPlay;
            playPause.Set(wantsToPlay ? PlayerText.Pause : PlayerText.Play, wantsToPlay ? "pause" : "play");
        }
        var isError = s.Phase.Kind == PhaseKind.Error;
        if (lastFirstFrame == null && s.FirstFrameMs != null) awaitingFrame = false;
        lastFirstFrame = s.FirstFrameMs;
        if (isError) awaitingFrame = true;
        Cover(s.FirstFrameMs == null || awaitingFrame);
        var waiting = !isError && s.Phase.Kind is PhaseKind.Preparing or PhaseKind.Buffering;
        if (!waiting) waitingSince = -1;
        else if (waitingSince < 0) waitingSince = Environment.TickCount64;
        Waiting();
        if (isError)
        {
            if (errorShown != s.Phase) ShowError(s.Phase);
        }
        else if (error != null) HideError();
        // The end, with nothing next: back to where playing started.
        if (s.Phase.Kind == PhaseKind.Ended) OnUi(Close);
    }

    /// <summary>Black over the video plane: until the first picture, and after an error until the next one.</summary>
    void Cover(bool on)
    {
        if (covered == on) return;
        covered = on;
        coverFade?.Stop();
        if (on)
        {
            black.Opacity = 1;
            return;
        }
        coverFade = new Animation(PleyMotion.OverlayMs) { DefaultAlphaFunction = new AlphaFunction(AlphaFunction.BuiltinFunctions.EaseOut) };
        coverFade.AnimateTo(black, "Opacity", 0f);
        coverFade.Play();
    }

    /// <summary>The spinner, once playback has waited 500 ms; "Reconnecting…" under it while the server is out of reach.</summary>
    void Waiting()
    {
        var on = waitingSince >= 0 && Environment.TickCount64 - waitingSince >= SpinnerAfterMs;
        if (on && !spinner.Visibility)
        {
            spinner.Show();
            spin.Play();
        }
        else if (!on && spinner.Visibility)
        {
            spinner.Hide();
            spin.Stop();
        }
        if (on && model.State.Value.Reconnecting) reconnecting.Show();
        else reconnecting.Hide();
    }

    /// <summary>
    /// Black where the letterboxed picture isn't: the window is clear over the whole video plane,
    /// and the TV leaves the plane's bars empty, so without these the TV's own screens show there.
    /// Each bar reaches a pixel into the picture, so no seam shows between them.
    /// </summary>
    void Bars(int videoWidth, int videoHeight)
    {
        if (gone) return;
        foreach (var bar in bars.Children.ToList())
        {
            bars.Remove(bar);
            bar.Dispose();
        }
        if (videoWidth <= 0 || videoHeight <= 0) return;
        var w = Ui.Dp(PleyTv.CanvasWidth);
        var h = Ui.Dp(PleyTv.CanvasHeight);
        var scale = Math.Min(w / videoWidth, h / videoHeight);
        var left = (float)Math.Floor((w - videoWidth * scale) / 2);
        var top = (float)Math.Floor((h - videoHeight * scale) / 2);
        void Bar(float x, float y, float bw, float bh)
        {
            if (bw >= 1 && bh >= 1) bars.Add(new View { Position = new Position(x, y), Size = new Size(bw, bh), BackgroundColor = Color.Black });
        }
        if (top >= 1)
        {
            Bar(0, 0, w, top + 1);
            Bar(0, h - top - 1, w, top + 1);
        }
        if (left >= 1)
        {
            Bar(0, 0, left + 1, h);
            Bar(w - left - 1, 0, left + 1, h);
        }
    }

    void Tick()
    {
        if (gone) return;
        var now = Environment.TickCount64;
        if (shown && model.State.Value.Phase.WantsToPlay && hideAt > 0 && now >= hideAt && seeker.Target == null) ShowControls(false);
        Waiting();
        if (shown) Layout();
    }

    /// <summary>The times, the played part and the thumb, at the target while a seek is coming.</summary>
    void Layout()
    {
        var duration = model.Clock.Value.DurationMs;
        var position = model.LivePositionMs();
        var at = seeker.Target ?? position;
        elapsed.Text = PlayerText.Time(at);
        remaining.Text = PlayerText.Remaining(at, duration);
        clock.Text = DateTime.Now.ToString("t", CultureInfo.CurrentCulture);
        ends.Text = model.State.Value.Phase.Kind == PhaseKind.Paused
            ? PlayerText.EndsAt(DateTime.Now.AddMilliseconds(Math.Max(0, duration - position)).ToString("t", CultureInfo.CurrentCulture))
            : "";
        var width = track.SizeWidth;
        var fraction = duration > 0 ? Math.Max(0, Math.Min(1, at / (double)duration)) : 0;
        played.SizeWidth = (float)(width * Math.Max(0, Math.Min(1, position / (double)Math.Max(1, duration))));
        var cx = track.PositionX + (float)(width * fraction);
        var cy = Ui.Dp(TrackCentre);
        thumb.Position = new Position(cx - thumb.SizeWidth / 2, cy - thumb.SizeHeight / 2);
        thumbRing.Position = new Position(cx - thumbRing.SizeWidth / 2, cy - thumbRing.SizeHeight / 2);
        if (seeker.BadgeAt is long b)
        {
            badge.Text = PlayerText.Badge(b);
            badge.PositionX = Math.Max(track.PositionX, Math.Min(track.PositionX + width - badge.SizeWidth, cx - badge.SizeWidth / 2));
            badge.Show();
        }
        else badge.Hide();
    }

    // ---- keys

    /// <summary>The keys while the controls are hidden; shown, focus has them (Back is the sheet's). Media keys work whatever shows.</summary>
    bool OnKey(string key, bool down)
    {
        if (gone) return false;
        var media = MediaOf(key);
        if (media != null)
        {
            if (down) OnMedia(media.Value);
            return true;
        }
        if (shell.Top != this || error != null) return false;
        if (shown)
        {
            Poke();
            return false;
        }
        switch (key)
        {
            case "Return":
            case "KP_Enter":
                // On the way up, as OK selects elsewhere.
                if (!down) Ok();
                return true;
            case "Left":
            case "Right":
                if (down && !Busy)
                {
                    ShowControls(true, scrubber);
                    Step(key == "Left" ? -1 : 1);
                }
                return true;
            case "Up":
                if (down) ShowControls(true, scrubber);
                return true;
            case "Down":
                if (down) ShowControls(true, playPause.Item);
                return true;
        }
        return false;
    }

    enum MediaAction
    {
        Play,
        Pause,
        PlayPause,
        Rewind,
        Forward,
        Stop,
    }

    static MediaAction? MediaOf(string key) => key switch
    {
        "XF86AudioPlay" => MediaAction.Play,
        "XF86AudioPause" => MediaAction.Pause,
        "XF86AudioPlayPause" => MediaAction.PlayPause,
        "XF86AudioRewind" => MediaAction.Rewind,
        "XF86AudioForward" => MediaAction.Forward,
        "XF86AudioStop" => MediaAction.Stop,
        _ => null,
    };

    MediaAction? lastMedia;
    long lastMediaAt;

    /// <summary>A media key, from the window or the media key service: Play, Pause, fast forward and rewind (30 s), Stop.</summary>
    void OnMedia(MediaAction action)
    {
        if (gone) return;
        var now = Environment.TickCount64;
        if (lastMedia == action && now - lastMediaAt < MediaTwiceMs) return;
        lastMedia = action;
        lastMediaAt = now;
        // An error on screen: only Stop, which leaves.
        if (error != null && action != MediaAction.Stop) return;
        switch (action)
        {
            case MediaAction.Play:
                model.Play();
                break;
            case MediaAction.Pause:
                model.Pause();
                if (!shown && shell.Top == this) ShowControls(true, scrubber);
                break;
            case MediaAction.PlayPause:
                PlayPause();
                break;
            case MediaAction.Rewind:
                Jump(-3);
                break;
            case MediaAction.Forward:
                Jump(3);
                break;
            case MediaAction.Stop:
                Close();
                break;
        }
    }

    void OnMediaKey(object? sender, global::Tizen.System.MediaKeyEventArgs e)
    {
        if (e.Status != global::Tizen.System.MediaKeyEventArgs.KeyStatus.Pressed) return;
        MediaAction? action = e.Value switch
        {
            global::Tizen.System.MediaKeyEventArgs.KeyValue.Play => MediaAction.Play,
            global::Tizen.System.MediaKeyEventArgs.KeyValue.Pause => MediaAction.Pause,
            global::Tizen.System.MediaKeyEventArgs.KeyValue.PlayPause => MediaAction.PlayPause,
            global::Tizen.System.MediaKeyEventArgs.KeyValue.Rewind => MediaAction.Rewind,
            global::Tizen.System.MediaKeyEventArgs.KeyValue.FastForward => MediaAction.Forward,
            global::Tizen.System.MediaKeyEventArgs.KeyValue.Stop => MediaAction.Stop,
            _ => null,
        };
        if (action != null) OnUi(() => OnMedia(action.Value));
    }

    /// <summary>
    /// OK on the video or the scrubber: during or just after a seek it goes there, playing (paused,
    /// it plays from there); else it plays or pauses.
    /// </summary>
    void Ok()
    {
        if (seeker.Confirm())
        {
            if (!model.WantsToPlay) model.Play();
            Layout();
            return;
        }
        PlayPause();
    }

    void PlayPause()
    {
        if (Busy) return;
        if (model.WantsToPlay)
        {
            model.Pause();
            // Paused, the controls stay; playing again, they go after a while.
            if (shell.Top == this) ShowControls(true, shown ? null : scrubber);
        }
        else
        {
            model.Play();
            if (shown) Poke();
        }
        Layout();
    }

    void Step(int direction)
    {
        Poke();
        if (gate.Holding) seeker.Repeat(direction, Environment.TickCount64);
        else
        {
            seeker.Press(direction);
            Settle();
        }
        Layout();
    }

    /// <summary>Back and Forward: a step of 10 s each, as Left and Right; fast forward and rewind 30 s.</summary>
    void Jump(int steps)
    {
        if (Busy) return;
        seeker.Press(Math.Sign(steps), Seeker.StepMs * Math.Abs(steps));
        Settle();
        if (shown) Poke();
        Layout();
    }

    /// <summary>Presses in a row seek once they stop; the badge goes a little later.</summary>
    void Settle()
    {
        var press = seeker.Presses;
        After(Seeker.SettleMs, () => seeker.Settle(press));
        After(Seeker.BadgeMs, () =>
        {
            seeker.HideBadge(press);
            Layout();
        });
    }

    void Released()
    {
        if (gone) return;
        if (seeker.Release()) Layout();
    }

    void SeekTo(long ms)
    {
        // A while after a seek, OK plays and pauses again.
        var number = seeker.Seeks + 1;
        After(Seeker.ConfirmMs, () => seeker.Settled(number));
        model.SeekTo(ms);
    }

    /// <summary>Shows or hides the controls; shown, focus goes to <paramref name="focus"/> (where it was, when null and already shown).</summary>
    void ShowControls(bool on, FocusItem? focus = null)
    {
        if (on)
        {
            if (focus != null && Tree.Current != focus) Tree.Focus(focus);
            else if (Tree.Current == null) Tree.Focus(scrubber);
            Poke();
        }
        else Tree.Blur();
        if (shown == on) return;
        shown = on;
        fade?.Stop();
        fade = new Animation(PleyMotion.OverlayMs) { DefaultAlphaFunction = new AlphaFunction(on ? AlphaFunction.BuiltinFunctions.EaseOut : AlphaFunction.BuiltinFunctions.EaseIn) };
        fade.AnimateTo(controls, "Opacity", on ? 1f : 0f);
        fade.Play();
        Layout();
    }

    /// <summary>A key came: the controls stay a while longer.</summary>
    void Poke() => hideAt = Environment.TickCount64 + PlayerText.HideAfterMs;

    bool OnBack()
    {
        if (error == null && shown)
        {
            ShowControls(false);
            return true;
        }
        Close();
        return true;
    }

    /// <summary>The app went to the background: it pauses, as the TV takes its player back.</summary>
    void AppShown(bool visible)
    {
        if (!gone && !visible) model.Pause();
    }

    // ---- errors

    /// <summary>
    /// A playback that stopped (design spec §8): its heading, why, and "Try again" (focused) when it
    /// may help, with Back; else Back alone. Centred on black.
    /// </summary>
    void ShowError(PlayerPhase phase)
    {
        HideError();
        errorShown = phase;
        seeker.Cancel();
        ShowControls(false);
        var view = new View { Size = Canvas(), BackgroundColor = Color.Black };
        var width = PleyTv.CanvasWidth - 320;
        var heading = Text.Label(phase.Title ?? PlayerText.ErrorTitle, PleyType.PageTitle, PleyColors.Fg, width: width);
        heading.HorizontalAlignment = HorizontalAlignment.Center;
        var message = Text.Label(phase.Message ?? "", PleyType.Body, PleyColors.Muted, maxLines: 4, width: width);
        message.HorizontalAlignment = HorizontalAlignment.Center;
        message.VerticalAlignment = VerticalAlignment.Top;
        var messageHeight = Math.Min(Math.Max(message.GetHeightForWidth(Ui.Dp(width)), Text.LineHeight(PleyType.Body)), Text.LineHeight(PleyType.Body) * 4) / Ui.Scale;
        message.SizeHeight = Ui.Dp(messageHeight);
        var buttons = new ButtonRow(12);
        ButtonView first;
        if (phase.CanRetry)
        {
            first = buttons.Add(new ButtonView(PlayerText.TryAgain, ButtonKind.Pill, Retry, iconName: "refresh-cw"));
            buttons.Add(new ButtonView(PlayerText.BackLabel, ButtonKind.Text, Close));
        }
        else first = buttons.Add(new ButtonView(PlayerText.BackLabel, ButtonKind.Pill, Close, iconName: "arrow-left"));
        var block = PleyType.PageTitle.Line + 12 + messageHeight + 32 + PleyTv.ButtonHeight;
        var top = (PleyTv.CanvasHeight - block) / 2;
        heading.Position = new Position(Ui.Dp(160), Ui.Dp(top));
        message.Position = new Position(Ui.Dp(160), Ui.Dp(top + PleyType.PageTitle.Line + 12));
        buttons.Position = new Position(Ui.Dp((PleyTv.CanvasWidth - buttons.WidthDp) / 2), Ui.Dp(top + PleyType.PageTitle.Line + 12 + messageHeight + 32));
        view.Add(heading);
        view.Add(message);
        view.Add(buttons);
        View.Add(view);
        error = view;
        // Up and Down stay on the buttons: the controls under them are hidden.
        buttons.Group.Exit = d => d is Direction.Up or Direction.Down;
        errorButtons = buttons.Group;
        Root.Add(buttons.Group);
        Tree.Focus(first.Item);
    }

    void HideError()
    {
        errorShown = null;
        if (errorButtons != null)
        {
            if (Tree.Current?.Parent == errorButtons) Tree.Blur();
            Root.Remove(errorButtons);
            errorButtons = null;
        }
        if (error == null) return;
        error.Unparent();
        error.Dispose();
        error = null;
    }

    void Retry() => model.Retry();

    // ---- leaving

    void Close()
    {
        if (gone) return;
        shell.Pop(this);
    }

    /// <summary>Off the screen: the model reports where it stopped and ends the session; the TV's player is let go.</summary>
    void Ended()
    {
        if (gone) return;
        gone = true;
        tick.Stop();
        spin.Stop();
        keys.Dispose();
        try
        {
            global::Tizen.System.MediaKey.Event -= OnMediaKey;
        }
        catch (Exception)
        {
            // It never took.
        }
        gate.HoldEnded -= Released;
        shell.Shown -= AppShown;
        foreach (var w in watches) w.Dispose();
        watches.Clear();
        model.Close();
        closed();
    }

    void OnUi(Action action)
    {
        if (ui == null) action();
        else ui.Post(_ =>
        {
            if (!gone) action();
        }, null);
    }

    /// <summary>Runs <paramref name="action"/> after <paramref name="ms"/>, unless the player has gone by then.</summary>
    void After(long ms, Action action)
    {
        var timer = new Timer((uint)Math.Max(1, ms));
        timer.Tick += (_, _) =>
        {
            if (!gone) action();
            timer.Dispose();
            return false;
        };
        timer.Start();
    }

    /// <summary>A 40 dp arc in the accent, turning while the stream comes.</summary>
    static View Spinner()
    {
        var arc = new ArcVisual { Thickness = Ui.Dp(3), StartAngle = 0, SweepAngle = 270, Cap = ArcVisual.CapType.Round, MixColor = PleyColors.Accent };
        return new View { Size = new Size(Ui.Dp(40), Ui.Dp(40)), Background = arc.OutputVisualMap };
    }

    /// <summary>The black scrim behind the controls: 65 % at the top, 25 % through the middle, 90 % at the bottom.</summary>
    static View Scrim()
    {
        var offsets = new PropertyArray();
        var colors = new PropertyArray();
        foreach (var (at, alpha) in new[] { (0f, 0.65f), (0.22f, 0.25f), (0.55f, 0.25f), (0.78f, 0.7f), (1f, 0.9f) })
        {
            offsets.Add(new PropertyValue(at));
            colors.Add(new PropertyValue(new Color(0, 0, 0, alpha)));
        }
        var map = new PropertyMap();
        map.Add(Visual.Property.Type, new PropertyValue((int)Visual.Type.Gradient));
        map.Add(GradientVisualProperty.StartPosition, new PropertyValue(new Vector2(0, -0.5f)));
        map.Add(GradientVisualProperty.EndPosition, new PropertyValue(new Vector2(0, 0.5f)));
        map.Add(GradientVisualProperty.StopOffset, new PropertyValue(offsets));
        map.Add(GradientVisualProperty.StopColor, new PropertyValue(colors));
        return new View { Size = Canvas(), Background = map };
    }

    /// <summary>
    /// A button of the player (design spec §2.5): its glyph alone on the scrim; focused, the accent
    /// fills a control square behind it, 1.08 larger, and its label fades in 8 dp below.
    /// </summary>
    sealed class PlayerButton : View
    {
        readonly View box;
        readonly float iconSize;
        ImageView icon;
        readonly TextLabel label;

        public PlayerButton(string text, string iconName, float size, float iconSize, Action onClick)
        {
            this.iconSize = iconSize;
            Size = new Size(Ui.Dp(size), Ui.Dp(size));
            Item = new FocusItem(text) { Selected = onClick };
            box = new View
            {
                Size = new Size(Ui.Dp(size), Ui.Dp(size)),
                CornerRadius = Ui.Dp(PleyShape.ControlRadius),
                PivotPoint = global::Tizen.NUI.PivotPoint.Center,
                PositionUsesPivotPoint = false,
            };
            icon = Glyph(iconName);
            box.Add(icon);
            Add(box);
            label = Text.Label(text, PleyType.Caption, PleyColors.Fg, width: 200);
            label.HorizontalAlignment = HorizontalAlignment.Center;
            label.Position = new Position(Ui.Dp(size / 2 - 100), Ui.Dp(size + 8));
            label.Opacity = 0;
            Add(label);
            Item.FocusChanged += Look;
        }

        public FocusItem Item { get; }

        ImageView Glyph(string name)
        {
            var glyph = Icons.Icon(name, iconSize, PleyColors.Fg);
            var inset = (SizeWidth - Ui.Dp(iconSize)) / 2;
            glyph.Position = new Position(inset, inset);
            return glyph;
        }

        public void Set(string text, string iconName)
        {
            if (Disposed) return;
            label.Text = text;
            box.Remove(icon);
            icon.Dispose();
            icon = Glyph(iconName);
            icon.Color = Item.Focused ? PleyColors.OnAccent : PleyColors.Fg;
            box.Add(icon);
        }

        void Look(bool focused)
        {
            if (Disposed) return;
            box.BackgroundColor = focused ? PleyColors.Accent : Color.Transparent;
            icon.Color = focused ? PleyColors.OnAccent : PleyColors.Fg;
            var scale = focused ? PleyShape.RoundScale : 1f;
            box.Scale = new Vector3(scale, scale, 1);
            label.Opacity = focused ? 1 : 0;
        }
    }
}
