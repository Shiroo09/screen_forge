using System.IO;
using ScreenForge.Translate;

namespace ScreenForge.Tests;

/// <summary>
/// Writes the golden Lens protobuf request bytes the Rust port
/// (<c>rust/crates/sf-net</c>) must reproduce byte-for-byte.
/// </summary>
/// <remarks>
/// Only writes when <c>SF_WRITE_GOLDEN=1</c>. Regenerate with:
/// <code>SF_WRITE_GOLDEN=1 dotnet test ScreenForge.Tests -c Release --filter GoldenProtoFixture</code>
/// The UUID is fixed here; the Rust test uses the same constant.
/// </remarks>
public sealed class GoldenProtoFixture
{
    private const ulong FixedUuid = 1234567890123456789UL;

    [Fact]
    public void WriteGoldenProto()
    {
        if (Environment.GetEnvironmentVariable("SF_WRITE_GOLDEN") != "1")
            return;

        string dir = Path.Combine(RepoRoot(), "rust", "crates", "sf-net", "tests", "fixtures");
        Directory.CreateDirectory(dir);

        byte[] png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

        File.WriteAllBytes(
            Path.Combine(dir, "lens_translate.bin"),
            GoogleLensClient.BuildRequest(png, 8, 8, "tr", "en", FixedUuid));

        File.WriteAllBytes(
            Path.Combine(dir, "lens_ocr.bin"),
            GoogleLensClient.BuildOcrRequest(png, 8, 8, FixedUuid));
    }

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "ScreenForge.sln")))
            dir = dir.Parent;
        return dir?.FullName ?? throw new InvalidOperationException("ScreenForge.sln not found");
    }
}
