using System.Globalization;

namespace ScreenForge.Settings;

/// <summary>
/// Görsel çeviri: varsayılan ayarlardaki ana dildir.
/// Menüden seçilen dil süreç kapanana kadar (sonraki oturuma kadar) geçerli kalır; ayara yazılmaz.
/// </summary>
internal static class VisualTranslateRecents
{
    internal const int Max = 5;

    private static string? _sessionCode;

    internal static string NativeCode(AppSettings settings)
    {
        string code = (settings.TranslateNativeLanguage ?? "").Trim().ToLowerInvariant();
        if (TranslateLanguageDefaults.IsKnown(code))
            return code;
        return TranslateLanguageDefaults.MapUiCulture(CultureInfo.CurrentUICulture);
    }

    internal static string ActiveCode(AppSettings settings)
    {
        if (!string.IsNullOrWhiteSpace(_sessionCode) && TranslateLanguageDefaults.IsKnown(_sessionCode))
            return _sessionCode;
        return NativeCode(settings);
    }

    internal static void SetSession(AppSettings settings, string code)
    {
        code = (code ?? "").Trim().ToLowerInvariant();
        if (!TranslateLanguageDefaults.IsKnown(code))
            return;

        _sessionCode = string.Equals(code, NativeCode(settings), StringComparison.OrdinalIgnoreCase)
            ? null
            : code;
    }

    internal static void ClearSession() => _sessionCode = null;

    internal static void Remember(AppSettings settings, string code)
    {
        code = code.Trim().ToLowerInvariant();
        settings.RecentVisualLanguages ??= [];
        string native = NativeCode(settings);
        settings.RecentVisualLanguages.RemoveAll(c =>
            string.Equals(c, code, StringComparison.OrdinalIgnoreCase)
            || string.Equals(c, native, StringComparison.OrdinalIgnoreCase)
            || !TranslateLanguageDefaults.IsKnown(c));

        if (!string.Equals(code, native, StringComparison.OrdinalIgnoreCase)
            && TranslateLanguageDefaults.IsKnown(code))
            settings.RecentVisualLanguages.Insert(0, code);

        if (settings.RecentVisualLanguages.Count > Max)
            settings.RecentVisualLanguages.RemoveRange(Max, settings.RecentVisualLanguages.Count - Max);
    }

    internal static IReadOnlyList<string> Visible(AppSettings settings)
    {
        settings.RecentVisualLanguages ??= [];
        string active = ActiveCode(settings);
        var list = new List<string>();
        foreach (var code in settings.RecentVisualLanguages)
        {
            string c = code.Trim().ToLowerInvariant();
            if (!TranslateLanguageDefaults.IsKnown(c))
                continue;
            if (string.Equals(c, active, StringComparison.OrdinalIgnoreCase))
                continue;
            if (list.Contains(c))
                continue;
            list.Add(c);
            if (list.Count == Max)
                break;
        }
        return list;
    }
}
