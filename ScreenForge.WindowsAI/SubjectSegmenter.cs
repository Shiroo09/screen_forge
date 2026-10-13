using System.Runtime.InteropServices;
using Microsoft.Windows.AI;
using Microsoft.Windows.AI.Imaging;
using Microsoft.Windows.ApplicationModel.DynamicDependency;
using Windows.Graphics;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace ScreenForge.WindowsAI;

public readonly record struct HintPoint(int X, int Y);

/// <summary>
/// Windows AI Image Object Extractor. Fotoğraflar arka plan aracının kullandığı
/// konu maskesinin açık sistem karşılığı: Gray8, 255 konu.
/// </summary>
public static class SubjectSegmenter
{
    const uint RuntimeMajorMinor = 0x00010008;
    static readonly PackageVersion MinRuntime = new(8000, 994, 2142, 0);
    static readonly object BootstrapGate = new();
    static bool _bootstrapped;

    public static async Task<byte[]> ExtractForegroundAsync(
        byte[] bgra,
        int width,
        int height,
        IReadOnlyList<HintPoint> includePoints,
        IReadOnlyList<HintPoint> excludePoints,
        bool centerRectangle,
        CancellationToken cancellationToken = default)
    {
        if (width <= 0 || height <= 0)
            throw new ArgumentOutOfRangeException(nameof(width));
        if (bgra.Length < width * height * 4)
            throw new ArgumentException("BGRA tamponu görüntüden kısa.", nameof(bgra));

        cancellationToken.ThrowIfCancellationRequested();
        await EnsureReadyAsync().ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();

        var first = await RunAsync(bgra, width, height, includePoints, excludePoints, centerRectangle && includePoints.Count == 0)
            .ConfigureAwait(false);
        if (includePoints.Count > 0 || excludePoints.Count > 0 || !IsDegenerate(first))
            return first;

        var fallback = await RunAsync(
            bgra,
            width,
            height,
            new[] { new HintPoint(width / 2, height / 2) },
            CornerExcludes(width, height),
            centerRectangle: false).ConfigureAwait(false);
        return Mean(fallback) is var fb && Mean(first) is var a && Math.Abs(fb - 128) < Math.Abs(a - 128)
            ? fallback
            : first;
    }

    public static string DescribeReadiness()
    {
        try
        {
            EnsureBootstrap();
            return ImageObjectExtractor.GetReadyState().ToString();
        }
        catch (Exception ex)
        {
            return ex.GetType().Name + ": " + ex.Message;
        }
    }

    private static void EnsureBootstrap()
    {
        lock (BootstrapGate)
        {
            if (_bootstrapped)
                return;

            if (!Bootstrap.TryInitialize(RuntimeMajorMinor, "", MinRuntime, Bootstrap.InitializeOptions.None, out int hr))
                throw new InvalidOperationException(
                    "Windows Uygulama Çalışma Zamanı 1.8 açılamadı (0x" + hr.ToString("X8") + ").");

            _bootstrapped = true;
        }
    }

    private static async Task EnsureReadyAsync()
    {
        EnsureBootstrap();
        var state = ImageObjectExtractor.GetReadyState();
        if (state == AIFeatureReadyState.Ready)
            return;
        if (state != AIFeatureReadyState.NotReady)
            throw new InvalidOperationException(Explain(state));

        var result = await ImageObjectExtractor.EnsureReadyAsync();
        if (result.Status == AIFeatureReadyResultState.Success)
            return;

        var detail = result.ErrorDisplayText;
        if (string.IsNullOrWhiteSpace(detail))
            detail = result.ExtendedError?.Message ?? result.Status.ToString();
        throw new InvalidOperationException("Windows arka plan modeli hazır değil. " + detail);
    }

    private static async Task<byte[]> RunAsync(
        byte[] bgra,
        int width,
        int height,
        IReadOnlyList<HintPoint> includePoints,
        IReadOnlyList<HintPoint> excludePoints,
        bool centerRectangle)
    {
        using var bitmap = CreateBitmap(bgra, width, height);
        using var extractor = await ImageObjectExtractor.CreateWithSoftwareBitmapAsync(bitmap);

        RectInt32[]? rects = null;
        if (centerRectangle)
        {
            int rw = Math.Max(1, width / 2);
            int rh = Math.Max(1, height / 2);
            rects = new[] { new RectInt32((width - rw) / 2, (height - rh) / 2, rw, rh) };
        }

        PointInt32[]? include = includePoints.Count == 0
            ? null
            : includePoints.Select(p => new PointInt32(p.X, p.Y)).ToArray();
        PointInt32[]? exclude = excludePoints.Count == 0
            ? null
            : excludePoints.Select(p => new PointInt32(p.X, p.Y)).ToArray();

        var hint = new ImageObjectExtractorHint(rects, include, exclude);
        using var mask = extractor.GetSoftwareBitmapObjectMask(hint);
        return CopyGray(mask, width, height);
    }

    private static SoftwareBitmap CreateBitmap(byte[] bgra, int width, int height)
    {
        var writer = new DataWriter();
        writer.WriteBytes(bgra);
        var buffer = writer.DetachBuffer();
        return SoftwareBitmap.CreateCopyFromBuffer(
            buffer,
            BitmapPixelFormat.Bgra8,
            width,
            height,
            BitmapAlphaMode.Premultiplied);
    }

    private static byte[] CopyGray(SoftwareBitmap source, int expectedWidth, int expectedHeight)
    {
        SoftwareBitmap gray = source;
        SoftwareBitmap? converted = null;
        if (source.BitmapPixelFormat != BitmapPixelFormat.Gray8)
        {
            converted = SoftwareBitmap.Convert(source, BitmapPixelFormat.Gray8, BitmapAlphaMode.Ignore);
            gray = converted;
        }

        try
        {
            int w = gray.PixelWidth;
            int h = gray.PixelHeight;
            if (w != expectedWidth || h != expectedHeight)
                throw new InvalidOperationException($"Maske boyutu {w}×{h}, görüntü {expectedWidth}×{expectedHeight}.");

            var pixels = new byte[w * h];
            using var buffer = gray.LockBuffer(BitmapBufferAccessMode.Read);
            using var reference = buffer.CreateReference();
            var plane = buffer.GetPlaneDescription(0);
            var access = (IMemoryBufferByteAccess)(object)reference;
            access.GetBuffer(out IntPtr data, out _);
            for (int y = 0; y < h; y++)
                Marshal.Copy(data + y * plane.Stride, pixels, y * w, w);
            return pixels;
        }
        finally
        {
            converted?.Dispose();
        }
    }

    private static bool IsDegenerate(byte[] mask)
    {
        double mean = Mean(mask);
        return mean < 8 || mean > 247;
    }

    private static double Mean(byte[] mask)
    {
        long sum = 0;
        foreach (byte value in mask)
            sum += value;
        return mask.Length == 0 ? 0 : sum / (double)mask.Length;
    }

    private static HintPoint[] CornerExcludes(int width, int height)
    {
        int x = Math.Max(1, width / 25);
        int y = Math.Max(1, height / 25);
        return
        [
            new HintPoint(x, y),
            new HintPoint(width - 1 - x, y),
            new HintPoint(x, height - 1 - y),
            new HintPoint(width - 1 - x, height - 1 - y),
        ];
    }

    private static string Explain(AIFeatureReadyState state) => state switch
    {
        AIFeatureReadyState.NotSupportedOnCurrentSystem =>
            "Bu Windows kurulumu arka plan modelini desteklemiyor.",
        AIFeatureReadyState.DisabledByUser =>
            "Windows yapay zeka modelleri kapalı.",
        _ => "Windows arka plan modeli kullanılamıyor (" + state + ").",
    };

    [ComImport]
    [Guid("5b0d3235-4dba-4d44-865e-8f1d0e4fd04d")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IMemoryBufferByteAccess
    {
        void GetBuffer(out IntPtr buffer, out uint capacity);
    }
}
