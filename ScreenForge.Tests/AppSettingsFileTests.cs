using ScreenForge.Settings;

namespace ScreenForge.Tests;

/// <summary>
/// Ayar dosyasının okuma/yazma davranışı: atomik yazma, bozuk dosyada yedek,
/// yarım (tmp) dosya bırakmama.
/// </summary>
public sealed class AppSettingsFileTests
{
    private static string NewTempDir()
    {
        var dir = Path.Combine(Path.GetTempPath(), "sf-settings-tests-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        return dir;
    }

    [Fact]
    public void Save_ThenLoad_RoundTripsHotkeysAndFlags()
    {
        var dir = NewTempDir();
        try
        {
            string path = Path.Combine(dir, "settings.json");
            var settings = new AppSettings
            {
                ShowCursor = true,
                RegionHotkey = new HotkeyConfig { Key = "S", Modifiers = ModifierKeys.Control | ModifierKeys.Shift },
            };
            settings.QuickTranslateHotkey.Key = "T";
            settings.QuickTranslateHotkey.Modifiers = ModifierKeys.Control | ModifierKeys.Shift;

            settings.Save(path);
            var loaded = AppSettings.Load(path, out bool isFirstRun);

            Assert.False(isFirstRun);
            Assert.True(loaded.ShowCursor);
            Assert.Equal("S", loaded.RegionHotkey.Key);
            Assert.Equal(ModifierKeys.Control | ModifierKeys.Shift, loaded.RegionHotkey.Modifiers);
            Assert.Equal(ModifierKeys.Control | ModifierKeys.Shift, loaded.QuickTranslateHotkey.Modifiers);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void Load_WhenFileIsCorrupt_KeepsBackupAndReturnsDefaults()
    {
        var dir = NewTempDir();
        try
        {
            string path = Path.Combine(dir, "settings.json");
            File.WriteAllText(path, "{ bu geçerli bir JSON değil ");

            var loaded = AppSettings.Load(path, out bool isFirstRun);

            Assert.True(isFirstRun);
            Assert.True(loaded.RegionHotkey.IsValid);
            Assert.Equal("S", loaded.RegionHotkey.Key);
            Assert.True(File.Exists(path + ".corrupt"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void Save_LeavesNoTempFilesBehind()
    {
        var dir = NewTempDir();
        try
        {
            string path = Path.Combine(dir, "settings.json");
            new AppSettings().Save(path);
            new AppSettings { ShowCursor = true }.Save(path);

            Assert.True(File.Exists(path));
            Assert.Empty(Directory.GetFiles(dir, "*.tmp"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void Load_WhenFileMissing_StartsFresh()
    {
        var dir = NewTempDir();
        try
        {
            string path = Path.Combine(dir, "settings.json");

            var loaded = AppSettings.Load(path, out bool isFirstRun);

            Assert.True(isFirstRun);
            Assert.True(loaded.RegionHotkey.IsValid);
            Assert.False(File.Exists(path + ".corrupt"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }
}