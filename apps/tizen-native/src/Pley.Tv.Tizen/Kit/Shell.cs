using System;
using System.Collections.Generic;
using Pley.Tv.Core.Ui;
using Tizen.NUI;
using Tizen.NUI.BaseComponents;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// What the remote drives: a page, or a dialog over it. Each has its own focus, so focus in a
/// dialog never reaches the page behind it, and comes back where it was when the dialog goes.
/// </summary>
class Sheet
{
    public Sheet(string name, Axis axis = Axis.Column)
    {
        View = new View { WidthResizePolicy = ResizePolicyType.FillToParent, HeightResizePolicy = ResizePolicyType.FillToParent };
        Root = new FocusGroup(axis, name);
        Tree = new FocusTree(Root);
    }

    public View View { get; }
    public FocusGroup Root { get; }
    public FocusTree Tree { get; }

    /// <summary>Back: true when the sheet took it (closed a pad, went back a step).</summary>
    public Func<bool>? Back { get; set; }

    /// <summary>The sheet was taken off the screen: timers and requests stop.</summary>
    public event Action? Gone;

    internal void Leave() => Gone?.Invoke();
}

/// <summary>
/// The screen's sheets: one page, and dialogs over it. Back goes to the top one; a dialog that
/// doesn't take it closes, and a page that doesn't leaves the app.
/// </summary>
sealed class Shell
{
    readonly Window window;
    readonly List<Sheet> sheets = new List<Sheet>();

    public Shell(Window window, KeyRepeatGate gate, Action exit)
    {
        this.window = window;
        Input = new Input(window, () => Top?.Tree, gate) { Back = () => OnBack(exit) };
    }

    public Input Input { get; }

    public Sheet? Top => sheets.Count > 0 ? sheets[sheets.Count - 1] : null;

    /// <summary>Shows <paramref name="page"/> in place of everything on screen.</summary>
    public void Show(Sheet page)
    {
        foreach (var l in sheets) Remove(l);
        sheets.Clear();
        sheets.Add(page);
        window.Add(page.View);
    }

    /// <summary>Puts <paramref name="sheet"/> (a dialog) over what is on screen.</summary>
    public void Push(Sheet sheet)
    {
        sheets.Add(sheet);
        window.Add(sheet.View);
    }

    /// <summary>Takes <paramref name="sheet"/> off the screen; focus is again where it was under it.</summary>
    public void Pop(Sheet sheet)
    {
        if (!sheets.Remove(sheet)) return;
        Remove(sheet);
    }

    static void Remove(Sheet sheet)
    {
        sheet.Tree.Blur();
        sheet.View.Unparent();
        sheet.Leave();
        sheet.View.Dispose();
    }

    void OnBack(Action exit)
    {
        var top = Top;
        if (top == null)
        {
            exit();
            return;
        }
        if (top.Back?.Invoke() == true) return;
        if (sheets.Count > 1) Pop(top);
        else exit();
    }
}
