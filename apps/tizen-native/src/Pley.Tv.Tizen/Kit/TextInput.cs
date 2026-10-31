using System;
using Pley.Tv.Core.Ui;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// Text entry on TV (D-pad rule 7, design spec §2.11; the Android app's TvTextField), only for search
/// and server addresses. Focus lands on the field without opening the keyboard, so D-pad users pass
/// through it; OK opens the TV's keyboard, and Back, the keyboard's own action or its closing ends
/// typing, leaving focus on the field. Pads type into it too (<see cref="Type"/>).
///
/// 44 dp, Surface2, radius 4, Body. At rest a 1 dp Control edge; focused or typing a 2 dp
/// AccentBright edge, without scale (fields are wide).
/// </summary>
sealed class TextInput : View
{
    public const float Height = 44;

    readonly Input input;
    readonly TextField field;
    IDisposable? keys;
    bool editing;

    public TextInput(Input input, float width, string placeholder, InputMethod.PanelLayoutType layout = InputMethod.PanelLayoutType.Normal,
        InputMethod.ActionButtonTitleType action = InputMethod.ActionButtonTitleType.Search)
    {
        this.input = input;
        Size = new Size(Ui.Dp(width), Ui.Dp(Height));
        CornerRadius = Ui.Dp(PleyShape.FieldRadius);
        BackgroundColor = PleyColors.Surface2;
        BorderlineOffset = -1;
        field = new TextField
        {
            Size = new Size(Ui.Dp(width - 32), Ui.Dp(Height)),
            Position = new Position(Ui.Dp(16), 0),
            PixelSize = Ui.Dp(PleyType.Body.Size),
            TextColor = PleyColors.Fg,
            PlaceholderText = placeholder,
            PlaceholderTextColor = PleyColors.Muted,
            PrimaryCursorColor = PleyColors.AccentBright,
            FontStyle = Text.Weight(PleyType.Regular),
            VerticalAlignment = VerticalAlignment.Center,
            EnableSelection = false,
            Focusable = true,
        };
        var method = new InputMethod { PanelLayout = layout, ActionButton = action };
        field.InputMethodSettings = method.OutputMap;
        field.TextChanged += (s, e) => Changed?.Invoke(field.Text);
        field.KeyEvent += OnFieldKey;
        // The keyboard closed by its own key (Back on it, its close key): typing ends.
        field.GetInputMethodContext().StatusChanged += (s, e) =>
        {
            if (!e.StatusChanged && editing) Stop();
        };
        Add(field);
        Item = new FocusItem("field") { Selected = Start };
        Item.FocusChanged += _ => Look();
        Look();
    }

    public FocusItem Item { get; }

    /// <summary>What is typed changed, by the keyboard or a pad.</summary>
    public event Action<string>? Changed;

    /// <summary>The keyboard's action key (Go, Search) was pressed.</summary>
    public Action? Submitted { get; set; }

    public string Value
    {
        get => field.Text ?? "";
        set => field.Text = value;
    }

    /// <summary>Adds <paramref name="text"/> at the end, as a pad's key types it.</summary>
    public void Type(string text) => field.Text = Value + text;

    /// <summary>Deletes the last character, as a pad's Delete does.</summary>
    public void DeleteLast()
    {
        var v = Value;
        if (v.Length > 0) field.Text = v.Substring(0, v.Length - 1);
    }

    /// <summary>Opens the TV's keyboard on the field.</summary>
    void Start()
    {
        if (editing) return;
        editing = true;
        // The remote's keys go to the keyboard and the field; what they leave (Up, Down) moves nothing.
        keys = input.Push((key, down) =>
        {
            if (down && key is "XF86Back" or "Escape") Stop();
            return true;
        });
        FocusManager.Instance.SetCurrentFocusView(field);
        field.GetInputMethodContext().Activate();
        field.GetInputMethodContext().ShowInputPanel();
        Look();
    }

    void Stop()
    {
        if (!editing) return;
        editing = false;
        keys?.Dispose();
        keys = null;
        var context = field.GetInputMethodContext();
        context.HideInputPanel();
        context.Deactivate();
        FocusManager.Instance.ClearFocus();
        Look();
    }

    bool OnFieldKey(object source, KeyEventArgs e)
    {
        if (!editing || e.Key.State != Key.StateType.Down) return false;
        switch (e.Key.KeyPressedName)
        {
            case "Return":
            case "KP_Enter":
            case "Select":
                Stop();
                Submitted?.Invoke();
                return true;
            case "XF86Back":
            case "Escape":
                Stop();
                return true;
            default:
                return false;
        }
    }

    void Look()
    {
        var ring = editing || Item.Focused;
        BorderlineWidth = Ui.Dp(ring ? 2 : 1);
        BorderlineColor = ring ? PleyColors.AccentBright : PleyColors.Control;
    }

    protected override void Dispose(global::Tizen.NUI.DisposeTypes type)
    {
        if (Disposed) return;
        if (editing) Stop();
        base.Dispose(type);
    }
}
