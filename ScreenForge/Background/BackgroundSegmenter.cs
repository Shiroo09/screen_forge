using ScreenForge.WindowsAI;
using SkiaSharp;

namespace ScreenForge.Background;

public readonly record struct SegmentationResult(byte[] Mask, bool UsedLocalModel);

/// <summary>
/// Önce Windows AI konu ayırıcısını dener. Paketlenmemiş uygulamada sistem modeli
/// erişim vermezse U²-Net küçük modeline düşer.
/// </summary>
public static class BackgroundSegmenter
{
    static int _useLocal;

    public static async Task<SegmentationResult> ExtractAsync(
        SKBitmap bitmap,
        IReadOnlyList<HintPoint> includePoints,
        IReadOnlyList<HintPoint> excludePoints,
        bool centerRectangle,
        CancellationToken cancellationToken = default)
    {
        if (Volatile.Read(ref _useLocal) == 0)
        {
            try
            {
                var packed = BackgroundCompositor.PackBgra(bitmap);
                var mask = await SubjectSegmenter.ExtractForegroundAsync(
                    packed,
                    bitmap.Width,
                    bitmap.Height,
                    includePoints,
                    excludePoints,
                    centerRectangle,
                    cancellationToken).ConfigureAwait(false);
                return new SegmentationResult(mask, false);
            }
            catch (Exception ex) when (ex is not OperationCanceledException)
            {
                Interlocked.Exchange(ref _useLocal, 1);
            }
        }

        var local = await LocalSubjectSegmenter.ExtractAsync(bitmap, cancellationToken).ConfigureAwait(false);
        return new SegmentationResult(local, true);
    }
}
