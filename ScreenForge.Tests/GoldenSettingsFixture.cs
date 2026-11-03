using System.IO;
using ScreenForge.Settings;

namespace ScreenForge.Tests;

/// <summary>
/// Writes the golden settings JSON the Rust port (<c>rust/crates/sf-settings</c>)
/// must be able to read and must reproduce on save.
/// </summary>
/// <remarks>
/// Only writes when <c>SF_WRITE_GOLDEN=1</c>. Regenerate with:
/// <code>SF_WRITE_GOLDEN=1 dotnet test ScreenForge.Tests -c Release --filter GoldenSettingsFixture</code>
/// </remarks>
public sealed class GoldenSettingsFixture
{
    [Fact]
    public void WriteGoldenSettings()
    {
        if (Environment.GetEnvironmentVariable("SF_WRITE_GOLDEN") != "1")
            return;

        string dir = Path.Combine(RepoRoot(), "rust", "crates", "sf-settings", "tests", "fixtures");
        Directory.CreateDirectory(dir);

        // A settings object touched in every nested section, so the JSON exercises
        // the full shape (flags enum, string enums, nested objects, lists).
        var settings = new AppSettings
        {
            ShowCursor = true,
            AutoCopyLinkAfterUpload = false,
            AutoCloseUploadWindow = true,
            LaunchAtStartup = false,
            RegionHotkey = new HotkeyConfig { Key = "S", Modifiers = ModifierKeys.Control | ModifierKeys.Alt },
            QuickTranslateHotkey = new HotkeyConfig { Key = "D1", Modifiers = ModifierKeys.Control },
            SettingsRevision = 2,
            OutputFormat = ImageFormat.Webp,
            Quality = 88,
            SaveDirectory = "C:/shots",
            TranslateSourceLanguage = "en",
            TranslateTargetLanguage = "tr",
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
            RecentVisualLanguages = ["de", "fr"],
        };
        settings.Gif.Fps = 24;
        settings.Gif.ColorCount = 128;
        settings.Gif.Quantizer = "Octree";
        settings.Gif.UseGlobalPalette = true;
        settings.Gif.Dithering = true;
        settings.Video.Fps = 25;
        settings.Video.Quality = VideoQuality.Medium;
        settings.Presenter.PenWidth = 7;
        settings.Presenter.SpotlightRadius = 300;
        settings.Presenter.InkColors = ["#FFEA6F12", "#FF2563EB"];
        settings.ToolStyles.StrokeWidth = 6;
        settings.ToolStyles.TextAlignment = TextAlignmentMode.Center;
        settings.ToolStyles.StepShape = StepShape.Bubble;
        settings.Subtitle.Theme = "gunduz";
        settings.Subtitle.Opacity = 0.5;

        string path = Path.Combine(dir, "golden_settings.json");
        settings.Save(path);
    }

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "ScreenForge.sln")))
            dir = dir.Parent;
        return dir?.FullName ?? throw new InvalidOperationException("ScreenForge.sln not found");
    }
}
