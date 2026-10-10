namespace ScreenForge.Record;

/// <summary>Donanım kodlayıcı bu makinede kapanırsa denenecek sıra.</summary>
internal readonly record struct EncoderChoice(bool Hardware, bool Audio);

internal static class EncoderFallback
{
    public const int MfEShutdown = unchecked((int)0xC00D3E85);

    public static EncoderChoice[] Plan(bool wantAudio)
    {
        if (!wantAudio)
            return [new(true, false), new(false, false)];

        return
        [
            new(true, true),
            new(false, true),
            new(true, false),
            new(false, false),
        ];
    }

    /// <summary>
    /// <paramref name="index"/> başarısız olduktan sonraki deneme.
    /// Ses yazımı kodlayıcıyı kapattıysa sesli seçenekler atlanır.
    /// </summary>
    public static int Next(EncoderChoice[] plan, int index, bool audioFailed)
    {
        for (int i = index + 1; i < plan.Length; i++)
        {
            if (audioFailed && plan[i].Audio)
                continue;
            return i;
        }

        return -1;
    }

    public static bool IsShutdown(Exception ex)
    {
        for (Exception? cur = ex; cur != null; cur = cur.InnerException)
        {
            if (cur.HResult == MfEShutdown)
                return true;
            if (cur.Message.Contains("MF_E_SHUTDOWN", StringComparison.OrdinalIgnoreCase)
                || cur.Message.Contains("0xC00D3E85", StringComparison.OrdinalIgnoreCase))
                return true;
        }

        return false;
    }
}
