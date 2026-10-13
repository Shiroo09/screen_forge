using System.IO;
using SkiaSharp;
using ScreenForge.Editor;
using ScreenForge.Settings;

namespace ScreenForge.Background;

public readonly record struct BackgroundSavePlan(string Path, ImageFormat Format, bool ReplacesOriginal);

public static class BackgroundFiles
{
    public const string Argument = "--edit-background";

    public static string? ReadArgument(IReadOnlyList<string> args)
    {
        for (int i = 0; i < args.Count; i++)
        {
            var arg = args[i];
            if (arg.Equals(Argument, StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Count)
                    return args[i + 1].Trim('"');
                return null;
            }

            const string prefix = "--edit-background=";
            if (arg.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                return arg[prefix.Length..].Trim('"');
        }

        return null;
    }

    public static SKBitmap DecodeOriented(string path)
    {
        using var codec = SKCodec.Create(path) ?? throw new InvalidOperationException("Görüntü okunamadı.");
        var info = new SKImageInfo(codec.Info.Width, codec.Info.Height, SKColorType.Bgra8888, SKAlphaType.Premul);
        var bitmap = new SKBitmap(info);
        var result = codec.GetPixels(info, bitmap.GetPixels());
        if (result != SKCodecResult.Success && result != SKCodecResult.IncompleteInput)
        {
            bitmap.Dispose();
            throw new InvalidOperationException("Görüntü çözülemedi.");
        }

        if (codec.EncodedOrigin == SKEncodedOrigin.TopLeft)
            return bitmap;

        var oriented = Orient(bitmap, codec.EncodedOrigin);
        bitmap.Dispose();
        return oriented;
    }

    public static BackgroundSavePlan PlanOverwrite(string originalPath, bool hasTransparency)
    {
        var ext = Path.GetExtension(originalPath).ToLowerInvariant();
        bool alphaOk = ext is ".png" or ".webp";
        if (hasTransparency && !alphaOk)
        {
            var png = Path.ChangeExtension(originalPath, ".png");
            if (string.Equals(png, originalPath, StringComparison.OrdinalIgnoreCase))
                png = originalPath + ".png";
            return new BackgroundSavePlan(png, ImageFormat.Png, false);
        }

        var format = ext switch
        {
            ".jpg" or ".jpeg" => ImageFormat.Jpeg,
            ".webp" => ImageFormat.Webp,
            _ => ImageFormat.Png,
        };
        return new BackgroundSavePlan(originalPath, format, true);
    }

    public static void Write(SKBitmap bitmap, string path, ImageFormat format)
    {
        using var data = ImageExporter.Encode(bitmap, format, format == ImageFormat.Jpeg ? 92 : 90);
        var directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(directory))
            Directory.CreateDirectory(directory);

        var temp = path + ".screenforge.tmp";
        File.WriteAllBytes(temp, data.ToArray());
        File.Move(temp, path, overwrite: true);
    }

    private static SKBitmap Orient(SKBitmap source, SKEncodedOrigin origin)
    {
        bool swap = origin is SKEncodedOrigin.LeftTop or SKEncodedOrigin.RightTop
            or SKEncodedOrigin.RightBottom or SKEncodedOrigin.LeftBottom;
        int w = swap ? source.Height : source.Width;
        int h = swap ? source.Width : source.Height;
        var output = new SKBitmap(w, h, SKColorType.Bgra8888, SKAlphaType.Premul);
        using var canvas = new SKCanvas(output);
        canvas.Clear(SKColors.Transparent);

        switch (origin)
        {
            case SKEncodedOrigin.TopRight:
                canvas.Translate(w, 0);
                canvas.Scale(-1, 1);
                break;
            case SKEncodedOrigin.BottomRight:
                canvas.RotateDegrees(180, w / 2f, h / 2f);
                break;
            case SKEncodedOrigin.BottomLeft:
                canvas.Translate(0, h);
                canvas.Scale(1, -1);
                break;
            case SKEncodedOrigin.LeftTop:
                canvas.Translate(0, h);
                canvas.RotateDegrees(-90);
                break;
            case SKEncodedOrigin.RightTop:
                canvas.Translate(w, 0);
                canvas.RotateDegrees(90);
                break;
            case SKEncodedOrigin.RightBottom:
                canvas.Translate(w, 0);
                canvas.Scale(-1, 1);
                canvas.Translate(0, h);
                canvas.RotateDegrees(-90);
                break;
            case SKEncodedOrigin.LeftBottom:
                canvas.Translate(w, h);
                canvas.Scale(-1, 1);
                canvas.Translate(w, 0);
                canvas.RotateDegrees(90);
                break;
        }

        canvas.DrawBitmap(source, 0, 0);
        return output;
    }
}
