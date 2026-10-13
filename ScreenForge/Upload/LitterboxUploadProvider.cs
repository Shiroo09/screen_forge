using System.IO;
using System.Net;
using System.Net.Http;
using System.Text.RegularExpressions;

namespace ScreenForge.Upload;

/// <summary>
/// Görüntüyü Litterbox'a yükler. Kalıcı Catbox bu ağda 0 bayt dosya bırakıp beyaz sayfa açtığı için
/// aynı servisin 72 saat tutan geçici alanına gidiyoruz.
/// </summary>
public sealed class LitterboxUploadProvider : IUploadProvider
{
    private const string Endpoint = "https://litterbox.catbox.moe/resources/internals/api.php";
    private const string KeepFor = "72h";

    private const string BrowserUserAgent =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    public const long MaxBytes = 1024L * 1024 * 1024;

    private static readonly HttpClient Http = CreateClient();

    private static HttpClient CreateClient()
    {
        var client = new HttpClient { Timeout = TimeSpan.FromMinutes(15) };
        client.DefaultRequestVersion = HttpVersion.Version11;
        client.DefaultVersionPolicy = HttpVersionPolicy.RequestVersionOrLower;
        client.DefaultRequestHeaders.TryAddWithoutValidation("User-Agent", BrowserUserAgent);
        return client;
    }

    public static bool IsAllowedFile(string path, out string? error)
    {
        error = null;
        string ext = Path.GetExtension(path).ToLowerInvariant();
        if (ext is ".exe" or ".scr" or ".cpl" or ".jar" || ext.StartsWith(".doc", StringComparison.Ordinal))
        {
            error = "Bu dosya türü yüklenemiyor.";
            return false;
        }

        try
        {
            if (new FileInfo(path).Length > MaxBytes)
            {
                error = "Dosya boyutu 1 GB sınırını aşıyor.";
                return false;
            }
        }
        catch
        {
            error = "Dosya okunamadı.";
            return false;
        }

        return true;
    }

    public async Task<UploadResult> UploadAsync(byte[] imageBytes, string mimeType, Action<double>? progress = null, string? fileName = null)
    {
        if (imageBytes == null || imageBytes.Length == 0)
            throw new ArgumentException("Geçersiz dosya verisi.");
        if (imageBytes.LongLength > MaxBytes)
            throw new InvalidOperationException("Dosya boyutu 1 GB sınırını aşıyor.");

        progress?.Invoke(0.2);

        string? url = await PostOnce(imageBytes, mimeType, fileName).ConfigureAwait(false);
        if (url == null)
        {
            progress?.Invoke(0.55);
            url = await PostOnce(imageBytes, mimeType, fileName).ConfigureAwait(false);
        }

        if (url == null)
            throw new InvalidOperationException("Yükleme reddedildi: Sunucu boş yanıt verdi.");

        progress?.Invoke(0.85);
        if (!await LinkHasBytes(url).ConfigureAwait(false))
            throw new InvalidOperationException("Dosya yüklenmiş göründü ama sunucuda boş duruyor.");

        progress?.Invoke(1.0);
        return new UploadResult(url, url);
    }

    private static async Task<string?> PostOnce(byte[] imageBytes, string mimeType, string? fileName)
    {
        using var form = new MultipartFormDataContent
        {
            { new StringContent("fileupload"), "reqtype" },
            { new StringContent(KeepFor), "time" },
        };
        var imageContent = new ByteArrayContent(imageBytes);
        imageContent.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue(
            string.IsNullOrWhiteSpace(mimeType) ? "application/octet-stream" : mimeType);
        form.Add(imageContent, "fileToUpload", SafeUploadName(fileName, mimeType));

        using var req = new HttpRequestMessage(HttpMethod.Post, Endpoint) { Content = form };
        req.Version = HttpVersion.Version11;
        req.VersionPolicy = HttpVersionPolicy.RequestVersionExact;
        req.Headers.ExpectContinue = false;

        using var resp = await Http.SendAsync(req).ConfigureAwait(false);
        string body = (await resp.Content.ReadAsStringAsync().ConfigureAwait(false)).Trim();
        if (!resp.IsSuccessStatusCode)
            throw new InvalidOperationException($"Yükleme sunucusu HTTP {(int)resp.StatusCode} döndürdü.");

        var match = Regex.Match(body, @"https?://\S+");
        if (match.Success)
            return match.Value;
        if (string.IsNullOrWhiteSpace(body))
            return null;

        string detail = body.Length > 180 ? body[..180] : body;
        throw new InvalidOperationException("Yükleme reddedildi: " + detail);
    }

    private static async Task<bool> LinkHasBytes(string url)
    {
        try
        {
            using var req = new HttpRequestMessage(HttpMethod.Head, url);
            req.Version = HttpVersion.Version11;
            req.VersionPolicy = HttpVersionPolicy.RequestVersionExact;
            using var resp = await Http.SendAsync(req).ConfigureAwait(false);
            if (!resp.IsSuccessStatusCode)
                return false;
            long? length = resp.Content.Headers.ContentLength;
            return length is null or > 0;
        }
        catch
        {
            return true;
        }
    }

    private static string SafeUploadName(string? fileName, string mimeType)
    {
        string name = string.IsNullOrWhiteSpace(fileName) ? "" : Path.GetFileName(fileName);
        name = name.Replace("\"", "").Replace("\r", "").Replace("\n", "").Trim();
        if (!string.IsNullOrWhiteSpace(name) && name != "." && name != "..")
            return name;

        string ext = mimeType.ToLowerInvariant() switch
        {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "png",
        };
        return "screenforge." + ext;
    }
}
