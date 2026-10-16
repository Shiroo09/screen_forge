using System.Drawing;
using ScreenForge.Capture;

namespace ScreenForge.Tests;

/// <summary>
/// Ekran yakalama yolu: DXGI önbelleği ile hızlı kopyalama, gerekirse BitBlt'e
/// düşme ve karıştırma hesabının doğruluğu.
/// </summary>
public sealed class ScreenCaptureTests
{
    [Fact]
    public void CaptureVirtualScreen_ReturnsVirtualScreenSizedBitmap()
    {
        var bounds = ScreenCapture.VirtualScreenBounds;
        Assert.True(bounds.Width > 0);
        Assert.True(bounds.Height > 0);

        using var bmp = ScreenCapture.CaptureVirtualScreen(includeCursor: false);

        Assert.Equal(bounds.Width, bmp.Width);
        Assert.Equal(bounds.Height, bmp.Height);
    }

    [Fact]
    public void Capture_ThenCrop_ProducesTheRequestedRegion()
    {
        // Kaydet / panoya kopyala / yükle yollarının kullandığı boru hattı.
        using var full = ScreenCapture.CaptureVirtualScreen(includeCursor: false);

        var region = new Rectangle(full.Width / 4, full.Height / 4, 320, 200);
        using var cropped = ScreenCapture.Crop(full, region);

        Assert.Equal(320, cropped.Width);
        Assert.Equal(200, cropped.Height);
    }

    [Fact]
    public void Crop_OutsideSource_ReturnsSmallestUsableBitmap()
    {
        using var full = ScreenCapture.CaptureVirtualScreen(includeCursor: false);

        using var cropped = ScreenCapture.Crop(full, new Rectangle(full.Width + 50, full.Height + 50, 80, 80));

        Assert.True(cropped.Width >= 1);
        Assert.True(cropped.Height >= 1);
    }

    [Fact]
    public void Capture_ThenToBitmapSource_KeepsPixelSize()
    {
        using var full = ScreenCapture.CaptureVirtualScreen(includeCursor: false);

        var source = ScreenCapture.ToBitmapSource(full);

        Assert.Equal(full.Width, source.PixelWidth);
        Assert.Equal(full.Height, source.PixelHeight);
    }

    [Fact]
    public void WarmupCapturePath_ThenCapture_StillMatchesVirtualScreen()
    {
        ScreenCapture.WarmupCapturePath();

        var bounds = ScreenCapture.VirtualScreenBounds;
        using var bmp = ScreenCapture.CaptureVirtualScreen(includeCursor: false);

        Assert.Equal(bounds.Width, bmp.Width);
        Assert.Equal(bounds.Height, bmp.Height);
    }

    [Fact]
    public void DxgiScreenCache_WhenAvailable_CoversVirtualScreen()
    {
        using var cache = DxgiScreenCache.TryCreate();
        if (cache is null)
            return; // DXGI masaüstü kopyalama yoksa BitBlt'e düşülür; davranış yine geçerli.

        Assert.Equal(ScreenCapture.VirtualScreenBounds, cache.Bounds);
    }

    [Fact]
    public void DxgiScreenCache_TryCapture_WritesIntoBitmap()
    {
        using var cache = DxgiScreenCache.TryCreate();
        if (cache is null)
            return;

        var bounds = cache.Bounds;
        using var bmp = new Bitmap(bounds.Width, bounds.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);

        Assert.True(cache.TryCapture(bmp));
    }

    [Fact]
    public void CompositeBlock_CopiesRowsAtGivenOffset()
    {
        // 2x2 kaynak, 4x3 hedef: kaynak (1,1) konumuna yerleşmeli.
        int srcW = 2, srcH = 2;
        var src = new byte[srcW * srcH * 4];
        for (int i = 0; i < src.Length; i++) src[i] = (byte)(i + 1);

        int dstStride = 4 * 4;
        var dst = new byte[dstStride * 3];

        DxgiScreenCache.CompositeBlock(src, srcW, srcH, dst, dstStride, destX: 1, destY: 1, destHeight: 3);

        int row0 = 1 * dstStride + 1 * 4;
        Assert.Equal(src[0], dst[row0]);
        Assert.Equal(src[3], dst[row0 + 3]);
        Assert.Equal(src[4], dst[row0 + 4]);
        int row1 = 2 * dstStride + 1 * 4;
        Assert.Equal(src[srcW * 4], dst[row1]);
        // Kaynak olmayan hücreler boş kalmalı.
        Assert.Equal(0, dst[0]);
        Assert.Equal(0, dst[dstStride]);
    }

    [Fact]
    public void CompositeBlock_SkipsRowsOutsideDestination()
    {
        int srcW = 2, srcH = 4;
        var src = new byte[srcW * srcH * 4];
        for (int i = 0; i < src.Length; i++) src[i] = 0xFF;

        int dstStride = 2 * 4;
        var dst = new byte[dstStride * 2];

        // Yalnızca hedefteki 2 satır (y=1..2) yazılmalı.
        DxgiScreenCache.CompositeBlock(src, srcW, srcH, dst, dstStride, destX: 0, destY: -1, destHeight: 2);

        Assert.Equal(0xFF, dst[0]);
        Assert.Equal(0xFF, dst[dstStride]);
    }
}