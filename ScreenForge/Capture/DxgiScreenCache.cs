using System.Drawing;
using System.Drawing.Imaging;
using ScreenForge.Record;

namespace ScreenForge.Capture;

/// <summary>
/// Ekran görüntüsünü BitBlt yerine DXGI masaüstü kopyalama ile alır: her çıktı için
/// bir kare kaynağı açık tutulur, değişiklik yoksa son bilinen görüntü kullanılır
/// (değişiklik yoksa ekran zaten aynıdır). Böylece ~50 ms'lik BitBlt yerine ~10-20 ms
/// yeterli olur. Oturum kilidi, korumalı içerik veya sürücü sorunlarında
/// <see cref="TryCapture"/> false döner ve çağıran BitBlt'e düşer.
/// </summary>
internal sealed class DxgiScreenCache : IDisposable
{
    private sealed class Entry
    {
        public Entry(DxgiFrameSource source, Rectangle dest, byte[] cache)
        {
            Source = source;
            Dest = dest;
            Cache = cache;
        }

        public DxgiFrameSource Source { get; }
        /// <summary>Çıktının sanal ekran içindeki yeri (fiziksel piksel).</summary>
        public Rectangle Dest { get; }
        /// <summary>Son bilinen BGRA görüntüsü.</summary>
        public byte[] Cache { get; }
    }

    private readonly List<Entry> _entries = new();
    private Rectangle _bounds;
    private bool _disposed;

    /// <summary>Kapladığı sanal ekran alanı (tüm çıktıların birleşimi).</summary>
    public Rectangle Bounds => _bounds;

    private DxgiScreenCache()
    {
    }

    /// <summary>
    /// Tüm monitörler için kopyalama kaynaklarını açar ve ilk kareleri toplar.
    /// Desteklenmiyorsa null (BitBlt'e düşülür).
    /// </summary>
    public static DxgiScreenCache? TryCreate()
    {
        var cache = new DxgiScreenCache();
        try
        {
            foreach (var screen in System.Windows.Forms.Screen.AllScreens)
            {
                var rect = screen.Bounds;
                if (rect.Width <= 0 || rect.Height <= 0) continue;

                var source = DxgiFrameSource.TryCreate(rect);
                if (source is null)
                {
                    cache.Dispose();
                    return null;
                }

                // Kısayol beklemez: yeni kare yoksa önbellek zaten günceldir,
                // uzun zaman aşımı her yakalamada 40 ms'lik boş bekleme olurdu.
                source.AcquireTimeoutMs = 2;

                cache._entries.Add(new Entry(source, rect, new byte[rect.Width * rect.Height * 4]));
                cache._bounds = cache._entries.Count == 1
                    ? rect
                    : Rectangle.Union(cache._bounds, rect);
            }

            if (cache._entries.Count == 0)
            {
                cache.Dispose();
                return null;
            }

            // İlk kareleri al: önbellek baştan dolu olsun.
            foreach (var entry in cache._entries)
            {
                if (!entry.Source.TryCopyBgra(entry.Cache, out _))
                {
                    cache.Dispose();
                    return null;
                }
            }

            return cache;
        }
        catch
        {
            cache.Dispose();
            return null;
        }
    }

    /// <summary>
    /// Sanal ekranın güncel görüntüsünü verilen bitmap'e yazar. Bitmap
    /// <see cref="Bounds"/> boyutunda ve 32 bpp ARGB olmalıdır. Başarısızsa false.
    /// </summary>
    public bool TryCapture(Bitmap dest)
    {
        if (_disposed) return false;
        try
        {
            foreach (var entry in _entries)
            {
                // Yeni kare geldiyse önbelleği tazele; gelmediyse önbellek zaten güncel.
                entry.Source.TryCopyBgra(entry.Cache, out _);
            }

            var data = dest.LockBits(new Rectangle(0, 0, dest.Width, dest.Height),
                ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
            try
            {
                unsafe
                {
                    var destSpan = new Span<byte>((void*)data.Scan0, data.Stride * dest.Height);
                    foreach (var entry in _entries)
                    {
                        CompositeBlock(entry.Cache, entry.Dest.Width, entry.Dest.Height,
                            destSpan, data.Stride, entry.Dest.X - _bounds.X, entry.Dest.Y - _bounds.Y, dest.Height);
                    }
                }
            }
            finally
            {
                dest.UnlockBits(data);
            }

            return true;
        }
        catch
        {
            // Kopyalama bozulduysa kaynağı bırakıp BitBlt'e düşürelim.
            Dispose();
            return false;
        }
    }

    /// <summary>
    /// Bir çıktının BGRA satırlarını hedef tampona, verilen konumdan itibaren yazar.
    /// Dikdörtgen hedef dışına taşan satırlar atlanır. Karıştırma hesabı tek başına
    /// denetlenebilsin diye burada tutulur.
    /// </summary>
    internal static void CompositeBlock(
        byte[] source, int sourceWidth, int sourceHeight,
        Span<byte> dest, int destStride, int destX, int destY, int destHeight)
    {
        int rowBytes = sourceWidth * 4;
        if (rowBytes <= 0 || destStride < rowBytes) return;

        for (int y = 0; y < sourceHeight; y++)
        {
            int dstY = destY + y;
            if (dstY < 0 || dstY >= destHeight) continue;
            if (destX < 0 || destX * 4 + rowBytes > destStride) continue;

            int dstOffset = dstY * destStride + destX * 4;
            source.AsSpan(y * rowBytes, rowBytes).CopyTo(dest.Slice(dstOffset, rowBytes));
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        foreach (var entry in _entries)
            entry.Source.Dispose();
        _entries.Clear();
    }
}