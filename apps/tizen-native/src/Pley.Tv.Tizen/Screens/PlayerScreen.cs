using System;
using System.Globalization;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Pley.Tv.Core;
using Pley.Tv.Core.Api;
using Pley.Tv.Core.Net;
using Pley.Tv.Core.Player;
using Pley.Tv.Core.Ui;
using Pley.Tv.Tizen.Kit;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;
using Media = Tizen.Multimedia;
using Timer = Tizen.NUI.Timer;

namespace Pley.Tv.Tizen.Screens;

/// <summary>
/// The player (design spec Â§2.7; the Android app's TvPlayerScreen): a session on the server, its
/// stream on the TV's own player, on the video plane under the window. Hidden controls: OK plays
/// and pauses, Left and Right seek (<see cref="Seeker"/>), Up and Down show the controls. Shown:
/// the title and clock at the top; the times, scrubber and Back 10 Â· Play/Pause Â· Forward 10 at the
/// bottom, over a black scrim; they go 5 s after the last key while it plays. Back hides them,
/// then stops: the session reports where it stopped and ends.
/// </summary>
sealed class PlayerScreen : Sheet
{
    /// <summary>The controls go this long after the last key while playing.</summary>
    const int HideAfterMs = 5_000;
    /// <summary>Where it is goes to the server this often while playing; paused, less often.</summary>
    const long ProgressMs = 10_000, PausedHeartbeatMs = 30_000;
    const float TimesTop = 399, TrackCentre = 425, RowTop = 441, TrackHeight = 4, Thumb = 10, ThumbFocused = 12;
    const float Button = 36, PlayButton = 44, TransportIcon = 24, PlayIcon = 26, TransportGap = 16;

    readonly Shell shell;
    readonly AppState state;
    readonly PlayRequest request;
    readonly KeyRepeatGate gate;
    readonly Action closed;
    readonly SynchronizationContext? ui = SynchronizationContext.Current;
    readonly SessionReporter reporter = new SessionReporter();
    readonly Seeker seeker;
    readonly IDisposable keys;

    readonly View black;
    readonly View bars;
    readonly View spinner;
    readonly Animation spin;
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

    Media.Player? player;
    PlaybackSession? session;
    long durationMs;
    long lastPosition;
    long lastReport;
    bool shown;
    bool playing;
    bool started;
    bool gone;
    Animation? fade;
    long hideAt;

    public PlayerScreen(Shell shell, AppState state, PlayRequest request, KeyRepeatGate gate, Action closed) : base("player")
    {
        Covers = true;
        this.shell = shell;
        this.state = state;
        this.request = request;
        this.gate = gate;
        this.closed = closed;
        lastPosition = request.StartMs;
        seeker = new Seeker(Position, () => durationMs, SeekTo);

        // Black until the first picture: the video plane under it shows nothing of the page meanwhile.
        black = new View { Size = Canvas(), BackgroundColor = Color.Black };
        View.Add(black);
        bars = new View { Size = Canvas() };
        View.Add(bars);
        spinner = Spinner();
        spinner.Position = new Position(Ui.Dp(PleyTv.CanvasWidth / 2 - 20), Ui.Dp(PleyTv.CanvasHeight / 2 - 20));
        View.Add(spinner);
        spin = new Animation(1000) { Looping = true };
        spin.AnimateBy(spinner, "Orientation", new Rotation(new Radian(new Degree(360)), PositionAxis.Z));
        spin.Play();

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
        thumbRing = new View
        {
            Size = new Size(Ui.Dp(ThumbFocused + 2 * PleyShape.ThumbRingOffset + 2 * PleyShape.ThumbRingStroke), Ui.Dp(ThumbFocused + 2 * PleyShape.ThumbRingOffset + 2 * PleyShape.ThumbRingStroke)),
            CornerRadius = Ui.Dp((ThumbFocused + 2 * PleyShape.ThumbRingOffset + 2 * PleyShape.ThumbRingStroke) / 2),
            BorderlineWidth = Ui.Dp(PleyShape.ThumbRingStroke),
            BorderlineColor = PleyColors.AccentBright,
            BorderlineOffset = -1,
            BackgroundColor = Color.Transparent,
            Opacity = 0,
        };
        controls.Add(thumbRing);
        thumb = new View { Size = new Size(Ui.Dp(Thumb), Ui.Dp(Thumb)), CornerRadius = Ui.Dp(Thumb / 2), BackgroundColor = PleyColors.Fg };
        controls.Add(thumb);

        // Back 10 Â· Play/Pause Â· Forward 10, centred; the smaller ones' centres in line with Play/Pause's.
        var back10 = new PlayerButton("Back 10 seconds", "rotate-ccw", Button, TransportIcon, () => Jump(-1));
        playPause = new PlayerButton("Pause", "pause", PlayButton, PlayIcon, PlayPause);
        var forward10 = new PlayerButton("Forward 10 seconds", "rotate-cw", Button, TransportIcon, () => Jump(1));
        var rowWidth = Button * 2 + PlayButton + TransportGap * 2;
        var x = (PleyTv.CanvasWidth - rowWidth) / 2;
        back10.Position = new Position(Ui.Dp(x), Ui.Dp(RowTop + (PlayButton - Button) / 2));
        playPause.Position = new Position(Ui.Dp(x + Button + TransportGap), Ui.Dp(RowTop));
        forward10.Position = new Position(Ui.Dp(x + Button + TransportGap + PlayButton + TransportGap), Ui.Dp(RowTop + (PlayButton - Button) / 2));
        controls.Add(back10);
        controls.Add(playPause);
        controls.Add(forward10);
        View.Add(controls);

        // Focus: the scrubber over the row; Left and Right on the scrubber seek.
        scrubber = new FocusItem("scrubber")
        {
            Selected = Ok,
            OwnKey = (key, down) =>
            {
                if (key != "Left" && key != "Right") return false;
                if (down) Step(key == "Left" ? -1 : 1);
                return true;
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
        Back = OnBack;
        Gone += Ended;

        tick = new Timer(250);
        tick.Tick += (_, _) =>
        {
            Tick();
            return !gone;
        };
        tick.Start();
    }

    bool Disposed => gone;

    static Size Canvas() => new Size(Ui.Dp(PleyTv.CanvasWidth), Ui.Dp(PleyTv.CanvasHeight));

    static TextLabel RightAligned(TextStyle style, Color color, float top)
    {
        var label = Text.Label("", style, color, width: 240);
        label.HorizontalAlignment = HorizontalAlignment.End;
        label.Position = new Position(Ui.Dp(PleyTv.CanvasWidth - PleyTv.SafeHorizontal - 240), Ui.Dp(top));
        return label;
    }

    /// <summary>Shows the player and starts: the item's title, a session, its stream.</summary>
    public void Show()
    {
        shell.Push(this);
        _ = Start();
    }

    async Task Start()
    {
        var api = state.Api.Value;
        if (api == null)
        {
            Fail("You're signed out of your server.");
            return;
        }
        try
        {
            var item = await api.Item(request.ItemId);
            if (gone) return;
            title.Text = PlayerText.Title(item);
            line.Text = PlayerText.Line(item) ?? "";
            durationMs = item.DurationMs ?? 0;
            // What Settings' passthrough choices turn on or off goes with what the TV says it plays.
            var caps = TizenCaps.Of(Platform.Facts(), state.Settings.Value.Passthrough);
            var body = PlayRequests.Body(request, state.InstallId(), caps);
            session = await PlaybackSession.Start(api, request.ItemId, body, reporter);
            if (gone)
            {
                session.End(request.StartMs);
                return;
            }
            if (session.Info.DurationMs > 0) durationMs = session.Info.DurationMs;
            await Open(api.Url(session.Info.Url), request.StartMs);
        }
        catch (Exception e) when (!gone)
        {
            Fail(e is ApiException or NetworkException ? Errors.Message(e) : "The TV's player couldn't open it.");
        }
    }

    async Task Open(string url, long startMs)
    {
        var p = new Media.Player();
        player = p;
        p.Display = new Media.Display(Window.Instance);
        p.SetSource(new Media.MediaUriSource(url));
        p.PlaybackCompleted += (_, _) => OnUi(Completed);
        p.ErrorOccurred += (_, e) => OnUi(() => Fail($"The TV's player stopped: {e.Error}."));
        p.BufferingProgressChanged += (_, e) => OnUi(() => Buffering(e.Percent < 100));
        p.VideoStreamChanged += (_, e) => OnUi(() => Bars(e.Size.Width, e.Size.Height));
        await p.PrepareAsync();
        if (gone) return;
        p.DisplaySettings.Mode = Media.PlayerDisplayMode.LetterBox;
        try
        {
            var size = p.StreamInfo.GetVideoProperties().Size;
            Bars(size.Width, size.Height);
        }
        catch (Exception)
        {
            // No video (or not known yet): no bars until the stream says its size.
        }
        // The stream's timeline is the file's: a conversion that starts there still seeks there.
        if (startMs >= 1000) await p.SetPlayPositionAsync((int)startMs, false);
        if (gone) return;
        p.Start();
        playing = true;
        started = true;
        Buffering(false);
        Reveal(black, false);
        Report(force: true);
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
        var shownW = videoWidth * scale;
        var shownH = videoHeight * scale;
        var left = (float)Math.Floor((w - shownW) / 2);
        var top = (float)Math.Floor((h - shownH) / 2);
        void Bar(float x, float y, float bw, float bh)
        {
            if (bw < 1 || bh < 1) return;
            bars.Add(new View { Position = new Position(x, y), Size = new Size(bw, bh), BackgroundColor = Color.Black });
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

    void OnUi(Action action)
    {
        if (ui == null) action();
        else ui.Post(_ =>
        {
            if (!gone) action();
        }, null);
    }

    long Position()
    {
        if (player != null && started)
        {
            try
            {
                lastPosition = player.GetPlayPosition();
            }
            catch (Exception)
            {
                // Between states: the last one known.
            }
        }
        return lastPosition;
    }

    void Tick()
    {
        if (gone) return;
        var now = Environment.TickCount64;
        if (shown && playing && hideAt > 0 && now >= hideAt && seeker.Target == null) ShowControls(false);
        Report(force: false);
        if (shown) Layout();
    }

    /// <summary>The times, the played part and the thumb, at the target while a seek is coming.</summary>
    void Layout()
    {
        var position = Position();
        var at = seeker.Target ?? position;
        elapsed.Text = PlayerText.Time(at);
        remaining.Text = PlayerText.Remaining(at, durationMs);
        clock.Text = DateTime.Now.ToString("t", CultureInfo.CurrentCulture);
        ends.Text = playing ? "" : $"Ends at {DateTime.Now.AddMilliseconds(Math.Max(0, durationMs - position)).ToString("t", CultureInfo.CurrentCulture)}";
        var width = track.SizeWidth;
        var fraction = durationMs > 0 ? Math.Max(0, Math.Min(1, at / (double)durationMs)) : 0;
        played.SizeWidth = (float)(width * Math.Max(0, Math.Min(1, position / (double)Math.Max(1, durationMs))));
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

    void Report(bool force)
    {
        if (session == null || !started) return;
        var now = Environment.TickCount64;
        if (!force && now - lastReport < (playing ? ProgressMs : PausedHeartbeatMs)) return;
        lastReport = now;
        session.Progress(Position(), playing);
    }

    void Buffering(bool on)
    {
        if (gone) return;
        if (on) spinner.Show();
        else spinner.Hide();
    }

    /// <summary>The keys while the controls are hidden; shown, focus has them (Back is the sheet's).</summary>
    bool OnKey(string key, bool down)
    {
        if (gone || error != null) return false;
        var media = key switch
        {
            "XF86AudioPlay" => MediaAction.Play,
            "XF86AudioPause" => MediaAction.Pause,
            "XF86AudioPlayPause" => MediaAction.PlayPause,
            "XF86AudioRewind" => MediaAction.Rewind,
            "XF86AudioForward" => MediaAction.Forward,
            "XF86AudioStop" => MediaAction.Stop,
            _ => (MediaAction?)null,
        };
        if (media != null)
        {
            if (down) OnMedia(media.Value);
            return true;
        }
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
                if (down)
                {
                    ShowControls(true, scrubberFocus: true);
                    Step(key == "Left" ? -1 : 1);
                }
                return true;
            case "Up":
                if (down) ShowControls(true, scrubberFocus: true);
                return true;
            case "Down":
                if (down) ShowControls(true, scrubberFocus: false);
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

    /// <summary>The same media key from NUI and from the media key service this soon after counts once.</summary>
    const long MediaTwiceMs = 250;

    MediaAction? lastMedia;
    long lastMediaAt;

    /// <summary>A media key, from the window or the media key service: Play, Pause, fast forward and rewind (30 s), Stop.</summary>
    void OnMedia(MediaAction action)
    {
        var now = Environment.TickCount64;
        if (lastMedia == action && now - lastMediaAt < MediaTwiceMs) return;
        lastMedia = action;
        lastMediaAt = now;
        switch (action)
        {
            case MediaAction.Play:
                if (!playing) PlayPause();
                break;
            case MediaAction.Pause:
                if (playing) PlayPause();
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

    /// <summary>OK on the video or the scrubber: confirms a seek coming or just made, else plays or pauses.</summary>
    void Ok()
    {
        if (seeker.Confirm()) return;
        PlayPause();
    }

    void Step(int direction)
    {
        Poke();
        if (gate.Holding) seeker.Repeat(direction, Environment.TickCount64);
        else
        {
            seeker.Press(direction);
            var press = seeker.Presses;
            After(Seeker.SettleMs, () => seeker.Settle(press));
            After(Seeker.BadgeMs, () =>
            {
                seeker.HideBadge(press);
                Layout();
            });
        }
        Layout();
    }

    /// <summary>Back and Forward: a step of 10 s each, as Left and Right; fast forward and rewind 30 s.</summary>
    void Jump(int steps)
    {
        seeker.Press(Math.Sign(steps), Seeker.StepMs * Math.Abs(steps));
        var press = seeker.Presses;
        After(Seeker.SettleMs, () => seeker.Settle(press));
        After(Seeker.BadgeMs, () => seeker.HideBadge(press));
        if (shown) Poke();
        Layout();
    }

    void Released()
    {
        if (gone) return;
        if (seeker.Release()) Layout();
    }

    void SeekTo(long ms)
    {
        lastPosition = ms;
        var number = seeker.Seeks + 1;
        After(Seeker.ConfirmMs, () => seeker.Settled(number));
        var p = player;
        if (p == null || !started) return;
        spinner.Show();
        _ = Seek(p, ms);
    }

    async Task Seek(Media.Player p, long ms)
    {
        try
        {
            await p.SetPlayPositionAsync((int)Math.Max(0, ms), false);
        }
        catch (Exception)
        {
            // A seek the player refuses leaves it playing where it was.
        }
        if (gone) return;
        spinner.Hide();
        Report(force: true);
    }

    void PlayPause()
    {
        var p = player;
        if (p == null || !started) return;
        try
        {
            if (playing) p.Pause();
            else p.Start();
            playing = !playing;
        }
        catch (Exception)
        {
            return;
        }
        playPause.Set(playing ? "Pause" : "Play", playing ? "pause" : "play");
        // Paused, the controls stay; playing again, they go after a while.
        if (!playing) ShowControls(true, scrubberFocus: Tree.Current == null || Tree.Current == scrubber);
        else Poke();
        Report(force: true);
        Layout();
    }

    void ShowControls(bool on, bool scrubberFocus = true)
    {
        if (on && !shown)
        {
            if (scrubberFocus) Tree.Focus(scrubber);
            else Tree.Focus(playPause.Item);
        }
        else if (on && scrubberFocus && Tree.Current != scrubber) Tree.Focus(scrubber);
        if (!on) Tree.Blur();
        if (on) Poke();
        if (shown == on) return;
        shown = on;
        Reveal(controls, on);
        Layout();
    }

    /// <summary>A key came: the controls stay a while longer.</summary>
    void Poke() => hideAt = Environment.TickCount64 + HideAfterMs;

    void Reveal(View view, bool on)
    {
        if (view == controls) fade?.Stop();
        var a = new Animation(PleyMotion.OverlayMs) { DefaultAlphaFunction = new AlphaFunction(on ? AlphaFunction.BuiltinFunctions.EaseOut : AlphaFunction.BuiltinFunctions.EaseIn) };
        a.AnimateTo(view, "Opacity", on ? 1f : 0f);
        a.Play();
        if (view == controls) fade = a;
    }

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

    void Completed()
    {
        lastPosition = durationMs;
        Close();
    }

    /// <summary>Can't play: "Can't play this item", why, and Back, centred on black (design spec Â§8).</summary>
    void Fail(string why)
    {
        if (gone) return;
        StopPlayer();
        spinner.Hide();
        controls.Hide();
        black.Opacity = 1;
        error?.Unparent();
        error = new View { Size = Canvas(), BackgroundColor = Color.Black };
        var width = PleyTv.CanvasWidth - 320;
        var heading = Text.Label("Can't play this item", PleyType.PageTitle, PleyColors.Fg, width: width);
        heading.HorizontalAlignment = HorizontalAlignment.Center;
        heading.Position = new Position(Ui.Dp(160), Ui.Dp(190));
        error.Add(heading);
        var text = Text.Label(why, PleyType.Body, PleyColors.Muted, maxLines: 4, width: width);
        text.HorizontalAlignment = HorizontalAlignment.Center;
        text.Position = new Position(Ui.Dp(160), Ui.Dp(190 + PleyType.PageTitle.Line + 12));
        error.Add(text);
        var buttons = new ButtonRow();
        buttons.Add(new ButtonView("Back", ButtonKind.Pill, Close, iconName: "arrow-left"));
        buttons.Position = new Position(Ui.Dp((PleyTv.CanvasWidth - buttons.WidthDp) / 2), Ui.Dp(190 + PleyType.PageTitle.Line + 12 + PleyType.Body.Line * 4 + 32));
        error.Add(buttons);
        View.Add(error);
        Root.Clear();
        Root.Add(buttons.Group);
        Tree.Enter(buttons.Group);
        if (session != null && !session.Ended) session.End(lastPosition);
    }

    void Close()
    {
        if (gone) return;
        shell.Pop(this);
    }

    /// <summary>Off the screen: the session says where it stopped and ends; the TV's player is let go.</summary>
    void Ended()
    {
        if (gone) return;
        var position = Position();
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
        StopPlayer();
        session?.End(position);
        closed();
    }

    void StopPlayer()
    {
        var p = player;
        player = null;
        started = false;
        playing = false;
        if (p == null) return;
        try
        {
            if (p.State == Media.PlayerState.Playing || p.State == Media.PlayerState.Paused) p.Stop();
            if (p.State != Media.PlayerState.Idle) p.Unprepare();
        }
        catch (Exception)
        {
            // Already stopped.
        }
        p.Dispose();
    }

    static void After(long ms, Action action)
    {
        var timer = new Timer((uint)ms);
        timer.Tick += (_, _) =>
        {
            action();
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
    /// A button of the player (design spec Â§2.5): its glyph alone on the scrim; focused, the accent
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
