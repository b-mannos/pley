using System;
using System.Linq;
using System.Threading;
using Pley.Tv.Core;
using Tizen.System;

namespace Pley.Tv.Tizen.Kit;

/// <summary>
/// The drive plugged into the TV, as the TV mounts it ("/opt/media/USBDriveA1"): the first one
/// mounted, null while none is. It follows drives plugged in and pulled out as the app runs.
/// </summary>
sealed class Drives : IDisposable
{
    readonly SynchronizationContext? ui = SynchronizationContext.Current;
    readonly EventHandler changed;

    public Drives()
    {
        changed = (_, _) => Post(Look);
        try
        {
            StorageManager.SetChangedEvent(StorageArea.External, changed);
        }
        catch (Exception)
        {
            // Not told of drives coming and going: what is there now still shows.
        }
        Look();
    }

    /// <summary>The drive's folder; null while none is plugged in.</summary>
    public Signal<string?> Root { get; } = new Signal<string?>(null);

    void Look()
    {
        string? root = null;
        try
        {
            root = StorageManager.Storages
                .Where(s => s.StorageType == StorageArea.External && s.State is StorageState.Mounted or StorageState.MountedReadOnly)
                .Select(s => s.RootDirectory)
                .FirstOrDefault(r => !string.IsNullOrEmpty(r));
        }
        catch (Exception)
        {
            // The TV wouldn't say: none.
        }
        Root.Value = root;
    }

    void Post(Action action)
    {
        if (ui != null) ui.Post(_ => action(), null);
        else action();
    }

    public void Dispose()
    {
        try
        {
            StorageManager.UnsetChangedEvent(StorageArea.External, changed);
        }
        catch (Exception)
        {
            // Never set.
        }
    }
}
