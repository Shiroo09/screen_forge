using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using ScreenForge.Capture;
using ScreenForge.Gif;
using ScreenForge.Gif.Input;
using ScreenForge.Settings;

namespace ScreenForge.Record;

public sealed class VideoRecorder : IRecordingSession
{
    private readonly Rectangle _region;
    private readonly VideoSettings _settings;
    private string _tempPath;
    private readonly Stopwatch _stopwatch = new();
    private readonly object _gate = new();
    private readonly int _outWidth;
    private readonly int _outHeight;
    private readonly AudioMixer? _mixer;
    private readonly InputHook? _hook;

    private IFrameSource? _source;
    private MfH264Writer? _writer;
    private Thread? _thread;
    private ManualResetEventSlim? _resumeSignal;
    private volatile bool _running;
    private volatile bool _paused;
    private volatile bool _stopRequested;
    private long _graceUntilMs;
    private int _frameCount;
    private int _captureAttempts;
    private EncoderChoice[] _plan = [];
    private int _attempt;
    private int _targetIntervalMs;
    private bool _disposed;
    private bool _finished;
    private long _audioBytes;
    private int _clickX, _clickY;
    private int _clickArgb;
    private long _clickUntil;

    internal VideoRecorder(Rectangle pixelRegion, VideoSettings settings, AudioMixer? mixer = null)
    {
        _region = pixelRegion;
        _settings = settings;
        (_outWidth, _outHeight) = VideoGeometry.CapLongEdge(pixelRegion.Width, pixelRegion.Height);
        Fps = Math.Clamp(settings.Fps, 1, 60);
        _targetIntervalMs = Math.Max(1, (int)Math.Round(1000.0 / Fps));
        _tempPath = Path.Combine(Path.GetTempPath(), $"screenforge-{Guid.NewGuid():N}.mp4");
        _mixer = mixer;
        if (settings.HighlightClicks)
        {
            _hook = new InputHook();
            _hook.MouseActivity += OnMouse;
            _hook.Start(mouse: true, keyboard: false);
        }
    }

    public string? OutputPath { get; private set; }
    public Exception? Failure { get; private set; }
    public int Fps { get; }
    public int FrameCount => _frameCount;
    public TimeSpan Elapsed => _stopwatch.Elapsed;
    public bool IsPaused => _paused;
    public float SystemPeak => _mixer?.SystemPeak ?? 0;
    public float MicPeak => _mixer?.MicPeak ?? 0;
    public bool HasSystemAudio => _mixer?.HasSystem == true;
    public bool HasMic => _mixer?.HasMic == true;
    public double CaptureEfficiency
    {
        get
        {
            int attempts = _captureAttempts;
            if (attempts <= 0) return 1.0;
            double expected = _stopwatch.ElapsedMilliseconds / (double)_targetIntervalMs;
            return expected <= 0 ? 1.0 : Math.Min(1.0, attempts / expected);
        }
    }

    public event Action? StateChanged;
#pragma warning disable CS0067
    public event Action? LimitReached;
#pragma warning restore CS0067

    public void Start()
    {
        if (_running) return;
        _running = true;
        _paused = false;
        if (_mixer != null) { _mixer.Paused = false; _mixer.Enqueue = true; }
        _resumeSignal = new ManualResetEventSlim(true);
        _stopwatch.Restart();
        _thread = new Thread(CaptureLoop)
        {
            IsBackground = true,
            Priority = ThreadPriority.AboveNormal,
            Name = "ScreenForge video capture",
        };
        _thread.SetApartmentState(ApartmentState.STA);
        _thread.Start();
        StateChanged?.Invoke();
    }

    public void Pause()
    {
        if (!_running || _paused) return;
        _paused = true;
        if (_mixer != null) { _mixer.Paused = true; _mixer.Enqueue = false; }
        _resumeSignal?.Reset();
        _stopwatch.Stop();
        StateChanged?.Invoke();
    }

    public void Resume()
    {
        if (!_paused) return;
        _stopwatch.Start();
        _paused = false;
        if (_mixer != null) { _mixer.Paused = false; _mixer.Enqueue = true; }
        _resumeSignal?.Set();
        StateChanged?.Invoke();
    }

    public void Stop()
    {
        if (!_running && _thread == null) return;

        _stopRequested = true;
        if (_paused)
            _running = false;
        else
            _graceUntilMs = _stopwatch.ElapsedMilliseconds + 1000;

        _resumeSignal?.Set();
        var thread = _thread;
        if (thread != null && thread.IsAlive && thread != Thread.CurrentThread)
            thread.Join(TimeSpan.FromSeconds(6));
        _running = false;
        _thread = null;
        _resumeSignal?.Dispose();
        _resumeSignal = null;
        _stopwatch.Stop();
        FinishWriter();
        StateChanged?.Invoke();
    }

    public void Restart()
    {
        if (_disposed) return;

        _paused = false;
        if (_mixer != null)
        {
            _mixer.Paused = false;
            _mixer.Enqueue = false;
        }
        _running = false;
        _stopRequested = true;
        _graceUntilMs = 0;
        _resumeSignal?.Set();
        var thread = _thread;
        if (thread != null && thread.IsAlive && thread != Thread.CurrentThread)
        {
            thread.Join(TimeSpan.FromSeconds(15));
            if (thread.IsAlive)
                return;
        }
        _thread = null;
        _resumeSignal?.Dispose();
        _resumeSignal = null;
        _stopwatch.Reset();

        OutputPath = null;
        Failure = null;
        TryDeleteTemp();
        _tempPath = Path.Combine(Path.GetTempPath(), $"screenforge-{Guid.NewGuid():N}.mp4");
        _finished = false;
        _frameCount = 0;
        _captureAttempts = 0;
        _audioBytes = 0;
        _stopRequested = false;

        Start();
    }

    private void OnMouse(MouseGesture gesture)
    {
        if (gesture.Type is not (MouseEventType.LeftDown or MouseEventType.RightDown or MouseEventType.MiddleDown))
            return;
        _clickX = gesture.X;
        _clickY = gesture.Y;
        _clickUntil = Environment.TickCount64 + 180;
        _clickArgb = gesture.Type == MouseEventType.RightDown
            ? unchecked((int)0x78FF0000)
            : gesture.Type == MouseEventType.MiddleDown
                ? unchecked((int)0x7800FFFF)
                : unchecked((int)0x78FFFF00);
    }

    private void CaptureLoop()
    {
        byte[]? pixels = null;
        byte[]? last = null;
        byte[]? scaled = null;
        long pauseOffset = 0;
        long pauseStarted = 0;

        try
        {
            _source = (IFrameSource?)DxgiFrameSource.TryCreate(_region) ?? new GdiFrameSource(_region);
            int bitrate = VideoBitrate.BitsPerSecond(_settings.Quality, _outWidth, _outHeight);
            OpenWriter(bitrate);
            pixels = new byte[_region.Width * _region.Height * 4];
            last = new byte[pixels.Length];
            if (_outWidth != _region.Width || _outHeight != _region.Height)
                scaled = new byte[_outWidth * _outHeight * 4];
            using var timerRes = new TimerResolution(1);

            long nextFrameAt = 0;
            while (_running)
            {
                if (_stopRequested && _stopwatch.ElapsedMilliseconds >= _graceUntilMs)
                    break;
                if (_paused)
                {
                    if (pauseStarted == 0)
                        pauseStarted = _stopwatch.ElapsedMilliseconds;
                    _resumeSignal?.Wait(100);
                    if (!_paused && pauseStarted != 0)
                    {
                        pauseOffset += _stopwatch.ElapsedMilliseconds - pauseStarted;
                        pauseStarted = 0;
                    }
                    nextFrameAt = _stopwatch.ElapsedMilliseconds;
                    continue;
                }

                FlushAudio(pauseOffset);

                long now = _stopwatch.ElapsedMilliseconds;
                long wait = nextFrameAt - now;
                if (wait > 1)
                {
                    Thread.Sleep((int)Math.Min(wait, 50));
                    continue;
                }

                bool got = _source.TryCopyBgra(pixels, out _);
                _captureAttempts++;
                if (got)
                {
                    OverlayPointer(pixels);
                    Buffer.BlockCopy(pixels, 0, last, 0, pixels.Length);
                }
                else if (_frameCount == 0)
                {
                    nextFrameAt += _targetIntervalMs;
                    continue;
                }
                else
                {
                    Buffer.BlockCopy(last, 0, pixels, 0, pixels.Length);
                }

                byte[] frame = pixels;
                if (scaled != null)
                {
                    ScaleNearest(pixels, _region.Width, _region.Height, scaled, _outWidth, _outHeight);
                    frame = scaled;
                }

                long timestamp = Math.Max(0, (_stopwatch.ElapsedMilliseconds - pauseOffset) * 10_000);
                if (WriteVideoFrame(frame, timestamp))
                    Interlocked.Increment(ref _frameCount);

                nextFrameAt += _targetIntervalMs;
                long drift = _stopwatch.ElapsedMilliseconds - nextFrameAt;
                if (drift > _targetIntervalMs * 3)
                    nextFrameAt = _stopwatch.ElapsedMilliseconds + _targetIntervalMs;
            }

            FlushAudio(pauseOffset);
        }
        catch (Exception ex) when (_stopRequested && EncoderFallback.IsShutdown(ex))
        {
            // Durdurma yazarın kilidini alıp Finalize etti; uçuştaki örnek MF_E_SHUTDOWN döner.
        }
        catch (Exception ex)
        {
            Failure = DescribeFailure(ex);
            _running = false;
        }
        finally
        {
            try { _mixer?.FlushRemainder(); } catch { /* ignore */ }
            try { FlushAudio(0); } catch { /* ignore */ }
            FinishWriter();
            _source?.Dispose();
            _source = null;
        }
    }

    private void OpenWriter(int bitrate)
    {
        bool wantAudio = _mixer is { HasAudio: true };
        _plan = EncoderFallback.Plan(wantAudio);
        Exception? last = null;
        for (int i = 0; i < _plan.Length; i++)
        {
            if (TryCreateWriter(_plan[i], bitrate, out Exception? error))
            {
                _attempt = i;
                return;
            }

            last = error;
        }

        throw last ?? new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).");
    }

    private bool WriteVideoFrame(byte[] frame, long timestamp)
    {
        while (true)
        {
            try
            {
                return _writer!.WriteBgra(frame, timestamp);
            }
            catch (Exception) when (_frameCount < 3 && !_stopRequested)
            {
                // Filtre değil gövde: WriteBgra kilidi bırakıldıktan sonra yeni yazar açılsın.
                if (!Recover(audioFailed: false))
                    throw;
            }
        }
    }

    private void FlushAudio(long pauseOffset)
    {
        if (_writer == null || _mixer == null) return;
        while (_mixer.TryDequeue(out var pcm))
        {
            if (_writer is not { HasAudio: true })
                continue;
            WriteAudioPacket(pcm, pauseOffset);
        }
    }

    private void WriteAudioPacket(byte[] pcm, long pauseOffset)
    {
        while (true)
        {
            if (_writer is not { HasAudio: true })
                return;

            long timestamp = _audioBytes * 10_000_000L / (AudioMixer.SampleRate * AudioMixer.BytesPerFrame);
            timestamp = Math.Max(0, timestamp - pauseOffset * 10_000);
            try
            {
                if (!_writer.WriteAudio(pcm, timestamp))
                    return;
                _audioBytes += pcm.Length;
                return;
            }
            catch (Exception) when (_frameCount < 3 && !_stopRequested)
            {
                if (!Recover(audioFailed: true))
                    throw;
            }
        }
    }

    /// <summary>
    /// Kodlayıcı ilk karelerde kapanırsa sıradaki seçeneği yeni bir dosyada dener.
    /// Eski yazarı, yenisi MF'yi ayakta tutana kadar bırakır; aksi halde süreç sayacı
    /// sıfırlanır ve yeni yazar da MF_E_SHUTDOWN alır.
    /// </summary>
    private bool Recover(bool audioFailed)
    {
        int bitrate = VideoBitrate.BitsPerSecond(_settings.Quality, _outWidth, _outHeight);
        var old = _writer;
        int next = EncoderFallback.Next(_plan, _attempt, audioFailed);
        while (next >= 0)
        {
            var choice = _plan[next];
            string path = Path.Combine(Path.GetTempPath(), $"screenforge-{Guid.NewGuid():N}.mp4");
            if (TryCreateWriter(choice, bitrate, out _, path))
            {
                _attempt = next;
                _audioBytes = 0;
                try { old?.Dispose(); } catch { /* broken encoder */ }
                TryDeleteTemp();
                _tempPath = path;
                return _writer != null;
            }

            next = EncoderFallback.Next(_plan, next, audioFailed);
        }

        try { old?.Dispose(); } catch { /* ignore */ }
        _writer = null;
        return false;
    }

    private bool TryCreateWriter(EncoderChoice choice, int bitrate, out Exception? error, string? path = null)
    {
        path ??= _tempPath;
        try
        {
            _writer = new MfH264Writer(path, _outWidth, _outHeight, Fps, bitrate, choice.Audio, choice.Hardware);
            error = null;
            return true;
        }
        catch (Exception ex)
        {
            error = ex;
            try { if (File.Exists(path)) File.Delete(path); } catch { /* locked */ }
            return false;
        }
    }

    private static Exception DescribeFailure(Exception ex)
    {
        if (ex.Message.Contains("H.264", StringComparison.OrdinalIgnoreCase))
            return new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).", ex);
        if (EncoderFallback.IsShutdown(ex))
        {
            return new InvalidOperationException(
                "Video kodlayıcı kapandı (MF_E_SHUTDOWN). Bu bilgisayarda hem donanım hem yazılım kodlayıcı kaydı sürdüremedi.",
                ex);
        }

        return ex;
    }

    private void OverlayPointer(byte[] bgra)
    {
        if (!_settings.CaptureCursor && !_settings.HighlightClicks)
            return;

        int w = _region.Width, h = _region.Height;
        using var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        try { CopyPacked(bgra, data, w, h); }
        finally { bmp.UnlockBits(data); }

        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.None;
            g.PixelOffsetMode = PixelOffsetMode.None;
            g.InterpolationMode = InterpolationMode.NearestNeighbor;
            if (_settings.CaptureCursor)
                ScreenCapture.DrawCursor(g, _region);
            if (_settings.HighlightClicks && Environment.TickCount64 < _clickUntil)
            {
                int x = _clickX - _region.X;
                int y = _clickY - _region.Y;
                using var brush = new SolidBrush(System.Drawing.Color.FromArgb(_clickArgb));
                g.FillEllipse(brush, x - 14, y - 14, 28, 28);
            }
        }

        data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        try { CopyPacked(data, bgra, w, h); }
        finally { bmp.UnlockBits(data); }
    }

    private static void CopyPacked(byte[] packed, BitmapData data, int w, int h)
    {
        int srcStride = w * 4;
        for (int y = 0; y < h; y++)
            System.Runtime.InteropServices.Marshal.Copy(packed, y * srcStride, data.Scan0 + y * data.Stride, srcStride);
    }

    private static void CopyPacked(BitmapData data, byte[] packed, int w, int h)
    {
        int dstStride = w * 4;
        for (int y = 0; y < h; y++)
            System.Runtime.InteropServices.Marshal.Copy(data.Scan0 + y * data.Stride, packed, y * dstStride, dstStride);
    }

    private static void ScaleNearest(byte[] src, int sw, int sh, byte[] dst, int dw, int dh)
    {
        for (int y = 0; y < dh; y++)
        {
            int sy = y * sh / dh;
            int srcRow = sy * sw * 4;
            int dstRow = y * dw * 4;
            for (int x = 0; x < dw; x++)
            {
                int sx = x * sw / dw;
                int si = srcRow + sx * 4;
                int di = dstRow + x * 4;
                dst[di] = src[si];
                dst[di + 1] = src[si + 1];
                dst[di + 2] = src[si + 2];
                dst[di + 3] = src[si + 3];
            }
        }
    }

    private void FinishWriter()
    {
        lock (_gate)
        {
            if (_finished) return;
            _finished = true;
            try { _writer?.Finish(); }
            catch (Exception ex) { Failure ??= ex; }
            try { _writer?.Dispose(); } catch { /* ignore */ }
            _writer = null;

            if (Failure == null && File.Exists(_tempPath) && _frameCount > 0)
                OutputPath = _tempPath;
            else
                TryDeleteTemp();
        }
    }

    private void TryDeleteTemp()
    {
        try
        {
            if (File.Exists(_tempPath))
                File.Delete(_tempPath);
        }
        catch { /* temp leftover */ }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
        _hook?.Dispose();
        _mixer?.Dispose();
        if (OutputPath == null)
            TryDeleteTemp();
    }
}
