using ScreenForge.Background;
using ScreenForge.Settings;
using SkiaSharp;

namespace ScreenForge.Tests;

public sealed class BackgroundEditTests
{
    [Fact]
    public void Remove_UsesMaskAsAlpha()
    {
        using var source = Solid(2, 1, new SKColor(10, 20, 30, 255));
        byte[] mask = [255, 0];

        using var result = BackgroundCompositor.Apply(
            source, mask, 2, 1, new BackgroundStyle(BackgroundMode.Remove, SKColors.White, null));

        Assert.Equal(255, result.GetPixel(0, 0).Alpha);
        Assert.Equal(10, result.GetPixel(0, 0).Red);
        Assert.Equal(0, result.GetPixel(1, 0).Alpha);
    }

    [Fact]
    public void Color_ReplacesOnlyBackground()
    {
        using var source = Solid(2, 1, new SKColor(10, 20, 30, 255));
        byte[] mask = [255, 0];
        var green = new SKColor(0, 200, 0);

        using var result = BackgroundCompositor.Apply(
            source, mask, 2, 1, new BackgroundStyle(BackgroundMode.Color, green, null));

        Assert.Equal(new SKColor(10, 20, 30, 255), result.GetPixel(0, 0));
        Assert.Equal(green, result.GetPixel(1, 0));
    }

    [Fact]
    public void ReadArgument_AcceptsSeparateAndEqualsForms()
    {
        Assert.Equal(@"C:\a b.png", BackgroundFiles.ReadArgument(["--edit-background", @"C:\a b.png"]));
        Assert.Equal(@"C:\a.png", BackgroundFiles.ReadArgument(["--edit-background=C:\\a.png"]));
        Assert.Null(BackgroundFiles.ReadArgument(["--other"]));
    }

    [Fact]
    public void PlanOverwrite_WritesPngBesideJpegWhenTransparent()
    {
        var plan = BackgroundFiles.PlanOverwrite(@"C:\photos\shot.jpg", hasTransparency: true);

        Assert.False(plan.ReplacesOriginal);
        Assert.Equal(ImageFormat.Png, plan.Format);
        Assert.Equal(@"C:\photos\shot.png", plan.Path);
    }

    [Fact]
    public void PlanOverwrite_ReplacesPngInPlace()
    {
        var plan = BackgroundFiles.PlanOverwrite(@"C:\photos\shot.png", hasTransparency: true);

        Assert.True(plan.ReplacesOriginal);
        Assert.Equal(@"C:\photos\shot.png", plan.Path);
    }

    [Fact]
    public void YerelModel_ParlakKonuyuKoyuZemindenAyirir()
    {
        using var source = new SKBitmap(96, 96, SKColorType.Bgra8888, SKAlphaType.Opaque);
        source.Erase(SKColors.Black);
        using (var canvas = new SKCanvas(source))
            canvas.DrawRect(new SKRect(28, 28, 68, 68), new SKPaint { Color = SKColors.White });

        var mask = LocalSubjectSegmenter.Extract(source);

        Assert.Equal(96 * 96, mask.Length);
        Assert.True(mask[48 * 96 + 48] > mask[2 * 96 + 2] + 40);
    }

    private static SKBitmap Solid(int width, int height, SKColor color)
    {
        var bitmap = new SKBitmap(width, height, SKColorType.Bgra8888, SKAlphaType.Premul);
        bitmap.Erase(color);
        return bitmap;
    }
}
