using System.IO;
using System.Windows;
using ScreenForge.Gif.Encoder;

namespace ScreenForge.Tests;

/// <summary>
/// Writes the golden GIF fixtures the Rust port (<c>rust/crates/sf-gif</c>) is
/// checked against byte-for-byte.
/// </summary>
/// <remarks>
/// Not a normal test: it only writes when <c>SF_WRITE_GOLDEN=1</c>, so a plain
/// <c>dotnet test</c> run never rewrites the committed fixtures. Regenerate with:
/// <code>
/// SF_WRITE_GOLDEN=1 dotnet test ScreenForge.Tests -c Release --filter GoldenGifFixture
/// </code>
/// The pixel generator here and <c>make_gradient</c> in the Rust test must stay
/// identical, or the comparison fails for the wrong reason.
/// </remarks>
public sealed class GoldenGifFixture
{
    [Fact]
    public void WriteGoldenFixtures()
    {
        if (Environment.GetEnvironmentVariable("SF_WRITE_GOLDEN") != "1")
            return;

        string dir = Path.Combine(RepoRoot(), "rust", "crates", "sf-gif", "tests", "fixtures");
        Directory.CreateDirectory(dir);

        var frames = Frames(32, 24);

        // Neural, local palette, no dithering.
        Write(Path.Combine(dir, "golden_neural_local.gif"), gif =>
        {
            gif.QuantizerType = QuantizerType.Neural;
            gif.SamplingFactor = 5;
            gif.MaximumNumberColor = 256;
            gif.UseGlobalPalette = false;
            gif.UseDithering = false;
            gif.SetCanvasSize(32, 24);
            foreach (var f in frames) gif.AddFrame(f, new Int32Rect(0, 0, 32, 24), 100, false);
        });

        // Octree, local palette.
        Write(Path.Combine(dir, "golden_octree_local.gif"), gif =>
        {
            gif.QuantizerType = QuantizerType.Octree;
            gif.MaximumNumberColor = 64;
            gif.UseGlobalPalette = false;
            gif.UseDithering = false;
            gif.SetCanvasSize(32, 24);
            foreach (var f in frames) gif.AddFrame(f, new Int32Rect(0, 0, 32, 24), 100, false);
        });

        // Neural + Floyd-Steinberg dithering.
        Write(Path.Combine(dir, "golden_neural_dither.gif"), gif =>
        {
            gif.QuantizerType = QuantizerType.Neural;
            gif.SamplingFactor = 5;
            gif.MaximumNumberColor = 32;
            gif.UseGlobalPalette = false;
            gif.UseDithering = true;
            gif.SetCanvasSize(32, 24);
            foreach (var f in frames) gif.AddFrame(f, new Int32Rect(0, 0, 32, 24), 100, false);
        });

        // Global palette built from the sample frames.
        Write(Path.Combine(dir, "golden_global.gif"), gif =>
        {
            gif.QuantizerType = QuantizerType.Neural;
            gif.SamplingFactor = 5;
            gif.MaximumNumberColor = 256;
            gif.UseGlobalPalette = true;
            gif.UseDithering = false;
            gif.SetCanvasSize(32, 24);
            gif.BuildGlobalPalette(frames);
            foreach (var f in frames) gif.AddFrame(f, new Int32Rect(0, 0, 32, 24), 100, false);
        });

        // Transparency: alpha-0 pixels must be written as the transparent index.
        Write(Path.Combine(dir, "golden_transparent.gif"), gif =>
        {
            gif.QuantizerType = QuantizerType.Octree;
            gif.MaximumNumberColor = 128;
            gif.UseGlobalPalette = false;
            gif.UseDithering = false;
            gif.SetCanvasSize(32, 24);
            var punched = (byte[])frames[0].Clone();
            for (int i = 3; i < punched.Length; i += 4) punched[i] = 0; // all transparent
            gif.AddFrame(punched, new Int32Rect(0, 0, 32, 24), 100, true);
            gif.AddFrame(frames[1], new Int32Rect(0, 0, 32, 24), 100, true);
        });
    }

    private static void Write(string path, Action<GifFile> build)
    {
        using var ms = new MemoryStream();
        using (var gif = new GifFile(ms))
            build(gif);
        File.WriteAllBytes(path, ms.ToArray());
    }

    /// <summary>Two deterministic BGRA frames; the second differs in a small square.</summary>
    private static List<byte[]> Frames(int width, int height)
    {
        var first = Gradient(width, height);
        var second = (byte[])first.Clone();
        for (int dy = 0; dy < 2; dy++)
        {
            for (int dx = 0; dx < 2; dx++)
            {
                int i = ((3 + dy) * width + 4 + dx) * 4;
                second[i] = 0;
                second[i + 1] = 0;
                second[i + 2] = 255;
                second[i + 3] = 255;
            }
        }
        return [first, second];
    }

    private static byte[] Gradient(int width, int height)
    {
        var pixels = new byte[width * height * 4];
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                int i = (y * width + x) * 4;
                pixels[i] = (byte)(x * 255 / Math.Max(1, width - 1));
                pixels[i + 1] = (byte)(y * 255 / Math.Max(1, height - 1));
                pixels[i + 2] = (byte)((x + y) * 127 / Math.Max(1, width + height - 2));
                pixels[i + 3] = 255;
            }
        }
        return pixels;
    }

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "ScreenForge.sln")))
            dir = dir.Parent;
        return dir?.FullName ?? throw new InvalidOperationException("ScreenForge.sln not found above the test binary");
    }
}
