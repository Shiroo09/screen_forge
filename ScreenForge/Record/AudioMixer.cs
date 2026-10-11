using System.Collections.Concurrent;
using NAudio.CoreAudioApi;
using NAudio.Wave;

namespace ScreenForge.Record;

internal sealed class AudioMixer : IDisposable
{
    public const int Channels = 2;
    public const int BytesPerFrame = 4;

    /// <summary>AAC'ye yazılan hız. 44.1 veya 48 kHz ise aygıtın kendi hızı, çevirme yok.</summary>
    public int SampleRate { get; private set; } = 48000;

    private readonly ConcurrentQueue<byte[]> _pcm = new();
    private readonly object _gate = new();
    private SystemLoopback? _sys;
    private WasapiLoopbackCapture? _loop;
    private WasapiCapture? _mic;
    private MMDevice? _micDevice;
    private WaveFormat? _loopFormat;
    private WaveFormat? _micFormat;
    private PcmPump? _sysPump;
    private PcmPump? _micPump;
    private bool _rateLocked;
    private float[] _sysRemain = [];
    private float[] _micRemain = [];
    private volatile float _sysPeak;
    private volatile float _micPeak;
    private volatile bool _paused;
    private volatile bool _enqueue;
    private bool _disposed;

    public AudioMixer(bool system, bool microphone, string? micDeviceId)
    {
        HasSystem = system;
        HasMic = microphone;
        if (system)
            StartLoopback();
        if (microphone)
            StartMic(micDeviceId);
    }

    public bool HasSystem { get; }
    public bool HasMic { get; }
    public bool HasAudio => _sys != null || _loop != null || _mic != null;
    public float SystemPeak => _sysPeak;
    public float MicPeak => _micPeak;

    public bool Paused
    {
        get => _paused;
        set => _paused = value;
    }

    public bool Enqueue
    {
        get => _enqueue;
        set => _enqueue = value;
    }

    public void SetMicrophone(string? deviceId)
    {
        lock (_gate)
        {
            StopMic();
            if (HasMic)
                StartMic(deviceId);
        }
    }

    public bool TryDequeue(out byte[] pcm) => _pcm.TryDequeue(out pcm!);

    public void FlushRemainder()
    {
        lock (_gate)
            DrainMix(force: true);
    }

    /// <summary>Kayıt başlayınca hız kilitlenir. Sonradan açılan mikrofon bu hıza çekilir.</summary>
    public void LockSampleRate()
    {
        lock (_gate)
        {
            EnsurePumps();
            _rateLocked = true;
        }
    }

    private void StartLoopback()
    {
        try
        {
            // Kulaklık (Bluetooth) uç noktası döngüsü paketi böler, ses titrer.
            // Uygulama karışımı kulaklığa inmeden alınır.
            var sys = new SystemLoopback();
            _sys = sys;
            _loopFormat = sys.WaveFormat;
            EnsurePumps();
            sys.DataAvailable += (_, e) => OnData(e, isMic: false);
            sys.Start();
        }
        catch
        {
            _sys?.Dispose();
            _sys = null;
            _loopFormat = null;
            _sysPump = null;
            try
            {
                _loop = new WasapiLoopbackCapture();
                _loopFormat = _loop.WaveFormat;
                EnsurePumps();
                _loop.DataAvailable += (_, e) => OnData(e, isMic: false);
                _loop.StartRecording();
            }
            catch
            {
                _loop?.Dispose();
                _loop = null;
                _loopFormat = null;
                _sysPump = null;
            }
        }
    }

    private void StartMic(string? deviceId)
    {
        try
        {
            var enumerator = new MMDeviceEnumerator();
            MMDevice device;
            if (!string.IsNullOrWhiteSpace(deviceId))
            {
                try { device = enumerator.GetDevice(deviceId); }
                catch { device = enumerator.GetDefaultAudioEndpoint(DataFlow.Capture, Role.Communications); }
            }
            else
            {
                device = enumerator.GetDefaultAudioEndpoint(DataFlow.Capture, Role.Communications);
            }

            _micDevice = device;
            _mic = new WasapiCapture(device, true, 50);
            _micFormat = _mic.WaveFormat;
            EnsurePumps();
            _mic.DataAvailable += (_, e) => OnData(e, isMic: true);
            _mic.StartRecording();
        }
        catch
        {
            StopMic();
        }
    }

    private void EnsurePumps()
    {
        if (!_rateLocked)
            SampleRate = ChooseRate();
        if (_loopFormat != null && (_sysPump == null || _sysPump.Target != SampleRate))
            _sysPump = new PcmPump(_loopFormat, SampleRate);
        if (_micFormat != null && (_micPump == null || _micPump.Target != SampleRate))
            _micPump = new PcmPump(_micFormat, SampleRate);
    }

    private int ChooseRate()
    {
        int loop = _loopFormat?.SampleRate ?? 0;
        int mic = _micFormat?.SampleRate ?? 0;
        // Sistem sesi kaydın duyulan kısmı. Onun hızı 44.1 veya 48 ise olduğu gibi yaz.
        if (loop is 44100 or 48000)
            return loop;
        if (mic is 44100 or 48000)
            return mic;
        return 48000;
    }

    private void OnData(WaveInEventArgs e, bool isMic)
    {
        if (_disposed || e.BytesRecorded <= 0)
            return;

        var pump = isMic ? _micPump : _sysPump;
        if (pump == null)
            return;

        float[] chunk;
        try
        {
            chunk = pump.Push(e.Buffer, e.BytesRecorded, out float peak);
            if (isMic) _micPeak = peak;
            else _sysPeak = peak;
        }
        catch
        {
            return;
        }

        // Kayıt dışındaki sesi biriktirme. Sayaç 0'a yığılır ve video uzar.
        if (_paused || !_enqueue || chunk.Length == 0)
            return;

        lock (_gate)
        {
            if (isMic) _micRemain = Concat(_micRemain, chunk);
            else _sysRemain = Concat(_sysRemain, chunk);
            DrainMix(force: false);
        }
    }

    private void DrainMix(bool force)
    {
        int sysFrames = _sysRemain.Length / Channels;
        int micFrames = _micRemain.Length / Channels;
        bool hasLoop = _sys != null || _loop != null;
        // Sistem sesi ana saat. Mikrofon gerideyse sisteme boşluk koyma.
        int frames = hasLoop ? sysFrames : micFrames;

        // AAC çerçevesi 1024 örnek. Yarım paketler kodlayıcıda titreme ve tekrar yapar.
        const int quantum = 1024;
        if (!force && frames < quantum)
            return;
        if (frames <= 0)
            return;
        if (!force)
            frames = frames / quantum * quantum;

        var pcm = new byte[frames * BytesPerFrame];
        for (int i = 0; i < frames; i++)
        {
            float l = 0, r = 0;
            if (i < sysFrames)
            {
                l += _sysRemain[i * 2];
                r += _sysRemain[i * 2 + 1];
            }
            if (i < micFrames)
            {
                l += _micRemain[i * 2];
                r += _micRemain[i * 2 + 1];
            }
            WritePcm16(pcm, i * 4, l, r);
        }

        _sysRemain = sysFrames > frames ? _sysRemain.AsSpan(frames * Channels).ToArray() : [];
        _micRemain = micFrames > frames ? _micRemain.AsSpan(frames * Channels).ToArray() : [];
        _pcm.Enqueue(pcm);
    }

    private static void WritePcm16(byte[] dest, int offset, float l, float r)
    {
        short sl = (short)Math.Clamp((int)(l * 32767f), short.MinValue, short.MaxValue);
        short sr = (short)Math.Clamp((int)(r * 32767f), short.MinValue, short.MaxValue);
        dest[offset] = (byte)sl;
        dest[offset + 1] = (byte)(sl >> 8);
        dest[offset + 2] = (byte)sr;
        dest[offset + 3] = (byte)(sr >> 8);
    }

    private static float[] Concat(float[] a, float[] b)
    {
        if (a.Length == 0) return b;
        var n = new float[a.Length + b.Length];
        a.CopyTo(n, 0);
        b.CopyTo(n, a.Length);
        return n;
    }

    private void StopMic()
    {
        try { _mic?.StopRecording(); } catch { /* ignore */ }
        _mic?.Dispose();
        _mic = null;
        _micDevice?.Dispose();
        _micDevice = null;
        _micFormat = null;
        _micPump = null;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        try { _sys?.Stop(); } catch { /* ignore */ }
        _sys?.Dispose();
        _sys = null;
        try { _loop?.StopRecording(); } catch { /* ignore */ }
        _loop?.Dispose();
        _loop = null;
        StopMic();
        while (_pcm.TryDequeue(out _)) { }
    }
}
