namespace ScreenForge.Upload;

public readonly record struct UploadResult(string Url, string ShareUrl);

/// <summary>
/// Görüntü yükleme sağlayıcısı soyutlaması. İlerideki backend'ler için genişletilebilir.
/// </summary>
public interface IUploadProvider
{
    /// <param name="imageBytes">Yüklenecek dosya.</param>
    /// <param name="mimeType">İçerik türü. Bilinmiyorsa application/octet-stream.</param>
    /// <param name="progress">0..1 arası ilerleme bildirimi (UI thread'e marshalling çağıran tarafın sorumluluğu).</param>
    /// <param name="fileName">Sunucuya gidecek dosya adı. Uzantı bundan okunur.</param>
    Task<UploadResult> UploadAsync(byte[] imageBytes, string mimeType, Action<double>? progress = null, string? fileName = null);
}
