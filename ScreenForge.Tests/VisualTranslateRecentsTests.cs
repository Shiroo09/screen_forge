using ScreenForge.Settings;

namespace ScreenForge.Tests;

public sealed class VisualTranslateRecentsTests
{
    public VisualTranslateRecentsTests() => VisualTranslateRecents.ClearSession();

    [Fact]
    public void ActiveCode_UsesNative_NotPair()
    {
        var settings = new AppSettings
        {
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
        };

        Assert.Equal("tr", VisualTranslateRecents.ActiveCode(settings));
    }

    [Fact]
    public void SetSession_OverridesUntilCleared_DoesNotReplaceNativeDefault()
    {
        var settings = new AppSettings
        {
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
        };

        VisualTranslateRecents.SetSession(settings, "de");
        Assert.Equal("de", VisualTranslateRecents.ActiveCode(settings));

        VisualTranslateRecents.SetSession(settings, "tr");
        Assert.Equal("tr", VisualTranslateRecents.ActiveCode(settings));
    }

    [Fact]
    public void Remember_SkipsNative_AndPutsLatestFirst()
    {
        var settings = new AppSettings
        {
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
        };

        VisualTranslateRecents.Remember(settings, "en");
        VisualTranslateRecents.Remember(settings, "de");
        VisualTranslateRecents.Remember(settings, "fr");

        Assert.Equal(["fr", "de", "en"], VisualTranslateRecents.Visible(settings));
    }

    [Fact]
    public void Visible_HidesRecentThatMatchesActive()
    {
        var settings = new AppSettings
        {
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
            RecentVisualLanguages = ["de", "fr"],
        };
        VisualTranslateRecents.SetSession(settings, "de");

        Assert.Equal(["fr"], VisualTranslateRecents.Visible(settings));
    }

    [Fact]
    public void Remember_CapsAtFive()
    {
        var settings = new AppSettings
        {
            TranslateNativeLanguage = "tr",
            TranslatePairLanguage = "en",
        };

        foreach (var code in new[] { "de", "fr", "es", "it", "pt", "ru" })
            VisualTranslateRecents.Remember(settings, code);

        Assert.Equal(5, settings.RecentVisualLanguages.Count);
        Assert.Equal("ru", settings.RecentVisualLanguages[0]);
        Assert.DoesNotContain("de", settings.RecentVisualLanguages);
    }
}
