using System.Drawing;
using System.Windows.Media.Imaging;
using ScreenForge.Settings;
using ScreenForge.Windows;

namespace ScreenForge.Tests;

/// <summary>
/// Hazır overlay penceresinin kısayol anındaki güncellenmesi: görüntü değişimi,
/// ekran sınırlarının güncellenmesi ve ön gösterim sonrası pencerenin gizli kalması.
/// </summary>
public sealed class CaptureOverlayScreenshotTests
{
    [Fact]
    public void SetScreenshot_ReplacesImageAndBounds()
    {
        WpfRunner.Run(() =>
        {
            var settings = new AppSettings();
            using var first = new Bitmap(64, 48, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            using var second = new Bitmap(32, 24, System.Drawing.Imaging.PixelFormat.Format32bppArgb);

            var win = new CaptureOverlayWindow(first, new Rectangle(0, 0, 64, 48), settings, CaptureMode.Region);
            win.SetScreenshot(second, new Rectangle(10, 20, 32, 24), CaptureMode.Region);

            Assert.Equal(new Rectangle(10, 20, 32, 24), win.VirtualBounds);

            var src = Assert.IsAssignableFrom<BitmapSource>(win.ScreenImage.Source);
            Assert.Equal(32, src.PixelWidth);
            Assert.Equal(24, src.PixelHeight);
        });
    }

    [Fact]
    public void SetScreenshot_KeepsTheNewBitmapAsScreenshot()
    {
        WpfRunner.Run(() =>
        {
            var settings = new AppSettings();
            using var first = new Bitmap(8, 8, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            using var second = new Bitmap(16, 16, System.Drawing.Imaging.PixelFormat.Format32bppArgb);

            var win = new CaptureOverlayWindow(first, new Rectangle(0, 0, 8, 8), settings, CaptureMode.Region);
            win.SetScreenshot(second, new Rectangle(0, 0, 16, 16), CaptureMode.Region);

            Assert.Equal(16, win.CurrentScreenshot.Width);
            Assert.Equal(16, win.CurrentScreenshot.Height);
        });
    }

    [Fact]
    public void SetScreenshot_DisposesThePreviousScreenshot()
    {
        WpfRunner.Run(() =>
        {
            var settings = new AppSettings();
            var first = new Bitmap(8, 8, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            using var second = new Bitmap(16, 16, System.Drawing.Imaging.PixelFormat.Format32bppArgb);

            var win = new CaptureOverlayWindow(first, new Rectangle(0, 0, 8, 8), settings, CaptureMode.Region);
            win.SetScreenshot(second, new Rectangle(0, 0, 16, 16), CaptureMode.Region);

            // Eski görüntü bırakılmış olmalı; aksi hâlde her kısayol tam ekran
            // görüntüsü kadar bellek sızdırırdı.
            Assert.ThrowsAny<Exception>(() => first.GetPixel(0, 0));
            Assert.Same(second, win.CurrentScreenshot);
        });
    }

    [Fact]
    public void PrepareForInstantShow_EndsWithHiddenWindowAtItsBounds()
    {
        WpfRunner.Run(() =>
        {
            var settings = new AppSettings();
            using var placeholder = new Bitmap(32, 24, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
            var bounds = new Rectangle(0, 0, 32, 24);

            var win = new CaptureOverlayWindow(placeholder, bounds, settings, CaptureMode.Region);
            win.PrepareForInstantShow();

            // Ön gösterim bittiğinde pencere gizli ve kendi sınırlarında olmalı.
            WpfRunner.DrainUntil(() => !win.IsVisible, 15000);

            Assert.False(win.IsVisible);
            Assert.Equal(bounds, win.VirtualBounds);
        });
    }
}