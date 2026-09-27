using System.IO;
using System.Threading;
using Velopack;
using Velopack.Sources;

namespace ScreenForge.Updates;

public static class AutoUpdateService
{
    private const string ReleasesUrl = "https://github.com/ruwiss/screen_forge/releases/latest/download";
    private static readonly string LogPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "ScreenForge",
        "update.log");

    private static readonly SemaphoreSlim Gate = new(1, 1);
    private static int _started;
    private static UpdateManager? _manager;
    private static UpdateInfo? _update;
    private static VelopackAsset? _ready;
    private static Action? _shutdown;
    private static string? _availableVersion;

    public static string? AvailableVersion => _availableVersion;

    public static void CheckOnStartup(Action shutdown)
    {
        _shutdown = shutdown;
        if (Interlocked.Exchange(ref _started, 1) == 1) return;
        _ = CheckAsync();
    }

    public static async Task ApplyAvailableAsync()
    {
        await Gate.WaitAsync().ConfigureAwait(false);
        try
        {
            var manager = _manager;
            var shutdown = _shutdown;
            if (manager == null || shutdown == null)
                return;

            VelopackAsset? asset = _ready;
            if (_update != null)
            {
                WriteLog($"İndirme başladı. Version={_update.TargetFullRelease.Version}");
                await manager.DownloadUpdatesAsync(_update).ConfigureAwait(false);
                asset = _update.TargetFullRelease;
                _ready = asset;
                _update = null;
            }

            if (asset == null)
                return;

            WriteLog($"Güncelleme uygulanacak. Version={asset.Version}");
            await Task.Delay(200).ConfigureAwait(false);
            manager.WaitExitThenApplyUpdates(asset, silent: true, restart: true);
            shutdown();
        }
        catch (Exception ex)
        {
            WriteLog("Güncelleme uygulama hatası: " + ex);
        }
        finally
        {
            Gate.Release();
        }
    }

    private static async Task CheckAsync()
    {
        await Gate.WaitAsync().ConfigureAwait(false);
        try
        {
            await Task.Delay(TimeSpan.FromSeconds(3)).ConfigureAwait(false);

            WriteLog($"Kontrol başladı. ProcessPath={Environment.ProcessPath}; BaseDirectory={AppContext.BaseDirectory}");

            var manager = new UpdateManager(new SimpleWebSource(ReleasesUrl));
            _manager = manager;
            WriteLog($"Velopack durumu. IsInstalled={manager.IsInstalled}");

            if (!manager.IsInstalled)
            {
                WriteLog("Güncelleme pas geçildi. Uygulama Velopack kurulumundan çalışmıyor.");
                return;
            }

            WriteLog($"Kurulu sürüm. CurrentVersion={manager.CurrentVersion}; PendingRestart={manager.UpdatePendingRestart != null}");

            if (manager.UpdatePendingRestart is { } pending)
            {
                _ready = pending;
                _availableVersion = pending.Version.ToString();
                WriteLog($"İndirilmiş güncelleme bekliyor. Version={_availableVersion}");
            }

            var update = await manager.CheckForUpdatesAsync().ConfigureAwait(false);
            if (update == null)
            {
                WriteLog(_availableVersion == null
                    ? "Yeni güncelleme bulunamadı."
                    : "Uzakta daha yeni sürüm yok. Bekleyen paket menüde kalacak.");
                return;
            }

            _update = update;
            _ready = null;
            _availableVersion = update.TargetFullRelease.Version.ToString();
            WriteLog($"Güncelleme bulundu. TargetVersion={_availableVersion}. Otomatik kurulmayacak.");
        }
        catch (Exception ex)
        {
            WriteLog("Güncelleme hatası: " + ex);
        }
        finally
        {
            Gate.Release();
        }
    }

    private static void WriteLog(string message)
    {
        try
        {
            var directory = Path.GetDirectoryName(LogPath);
            if (!string.IsNullOrEmpty(directory))
                Directory.CreateDirectory(directory);

            File.AppendAllText(LogPath, $"[{DateTimeOffset.Now:O}] {message}{Environment.NewLine}");
        }
        catch
        {
        }
    }
}
