using SkiaSharp;

namespace ScreenForge.Background;

public enum BackgroundMode
{
    Original,
    Remove,
    Color,
    Image,
}

public readonly record struct BackgroundStyle(
    BackgroundMode Mode,
    SKColor Color,
    SKBitmap? Replacement);

/// <summary>
/// Arka plan sonucu: kaldır (alfa), düz renkle değiştir veya başka görüntüyle kapla.
/// Maskedeki 255 konu, 0 arka plandır.
/// </summary>
public static class BackgroundCompositor
{
    public static SKBitmap Apply(SKBitmap source, ReadOnlySpan<byte> mask, int maskWidth, int maskHeight, BackgroundStyle style)
    {
        if (style.Mode == BackgroundMode.Original)
            return source.Copy();

        using var fitted = FitMask(mask, maskWidth, maskHeight, source.Width, source.Height);
        using var foreground = ApplyMaskAlpha(source, fitted);

        if (style.Mode == BackgroundMode.Remove)
            return foreground.Copy();

        var output = new SKBitmap(source.Width, source.Height, SKColorType.Bgra8888, SKAlphaType.Premul);
        using var canvas = new SKCanvas(output);
        canvas.Clear(SKColors.Transparent);

        if (style.Mode == BackgroundMode.Color)
        {
            canvas.Clear(style.Color);
        }
        else if (style.Mode == BackgroundMode.Image && style.Replacement != null)
        {
            DrawCover(canvas, style.Replacement, source.Width, source.Height);
        }
        else
        {
            canvas.Clear(SKColors.White);
        }

        canvas.DrawBitmap(foreground, 0, 0);
        return output;
    }

    public static bool HasTransparency(SKBitmap bitmap)
    {
        var pixels = bitmap.GetPixelSpan();
        for (int i = 3; i < pixels.Length; i += 4)
        {
            if (pixels[i] < 255)
                return true;
        }
        return false;
    }

    public static SKBitmap Downscale(SKBitmap source, int maxEdge)
    {
        int longest = Math.Max(source.Width, source.Height);
        if (longest <= maxEdge)
            return source.Copy();

        float scale = maxEdge / (float)longest;
        int w = Math.Max(1, (int)Math.Round(source.Width * scale));
        int h = Math.Max(1, (int)Math.Round(source.Height * scale));
        var info = new SKImageInfo(w, h, SKColorType.Bgra8888, SKAlphaType.Premul);
        return source.Resize(info, new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear))
            ?? throw new InvalidOperationException("Görüntü küçültülemedi.");
    }

    public static byte[] PackBgra(SKBitmap bitmap)
    {
        if (bitmap.ColorType != SKColorType.Bgra8888)
            throw new ArgumentException("BGRA bekleniyor.", nameof(bitmap));

        int w = bitmap.Width;
        int h = bitmap.Height;
        var packed = new byte[w * h * 4];
        var src = bitmap.GetPixelSpan();
        int row = bitmap.RowBytes;
        if (row == w * 4 && src.Length >= packed.Length)
        {
            src[..packed.Length].CopyTo(packed);
            return packed;
        }

        for (int y = 0; y < h; y++)
            src.Slice(y * row, w * 4).CopyTo(packed.AsSpan(y * w * 4, w * 4));
        return packed;
    }

    private static SKBitmap FitMask(ReadOnlySpan<byte> mask, int maskWidth, int maskHeight, int width, int height)
    {
        if (mask.Length < maskWidth * maskHeight)
            throw new ArgumentException("Maske tamponu kısa.", nameof(mask));

        var info = new SKImageInfo(maskWidth, maskHeight, SKColorType.Gray8, SKAlphaType.Opaque);
        var gray = new SKBitmap(info);
        var dst = gray.GetPixelSpan();
        int need = maskWidth * maskHeight;
        if (gray.RowBytes == maskWidth)
            mask[..need].CopyTo(dst);
        else
        {
            for (int y = 0; y < maskHeight; y++)
                mask.Slice(y * maskWidth, maskWidth).CopyTo(dst.Slice(y * gray.RowBytes, maskWidth));
        }

        if (maskWidth == width && maskHeight == height)
            return gray;

        var scaled = gray.Resize(new SKImageInfo(width, height, SKColorType.Gray8, SKAlphaType.Opaque),
            new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.None));
        gray.Dispose();
        return scaled ?? throw new InvalidOperationException("Maske ölçeklenemedi.");
    }

    private static SKBitmap ApplyMaskAlpha(SKBitmap source, SKBitmap grayMask)
    {
        var output = new SKBitmap(source.Width, source.Height, SKColorType.Bgra8888, SKAlphaType.Premul);
        var src = source.GetPixelSpan();
        var dst = output.GetPixelSpan();
        var mask = grayMask.GetPixelSpan();
        int sw = source.Width;
        int sh = source.Height;
        int sRow = source.RowBytes;
        int dRow = output.RowBytes;
        int mRow = grayMask.RowBytes;

        for (int y = 0; y < sh; y++)
        {
            int s = y * sRow;
            int d = y * dRow;
            int m = y * mRow;
            for (int x = 0; x < sw; x++)
            {
                byte coverage = mask[m + x];
                // Kaynak ön çarpımlı. Kapsamı dört kanala da uygula; aksi halde alfa 0 iken renk sızar.
                dst[d] = (byte)(src[s] * coverage / 255);
                dst[d + 1] = (byte)(src[s + 1] * coverage / 255);
                dst[d + 2] = (byte)(src[s + 2] * coverage / 255);
                dst[d + 3] = (byte)(src[s + 3] * coverage / 255);
                s += 4;
                d += 4;
            }
        }

        return output;
    }

    private static void DrawCover(SKCanvas canvas, SKBitmap image, int width, int height)
    {
        float scale = Math.Max(width / (float)image.Width, height / (float)image.Height);
        float dw = image.Width * scale;
        float dh = image.Height * scale;
        var dest = new SKRect((width - dw) / 2f, (height - dh) / 2f, (width + dw) / 2f, (height + dh) / 2f);
        canvas.DrawBitmap(image, dest);
    }
}
