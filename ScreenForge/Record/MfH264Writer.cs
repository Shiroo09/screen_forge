using System.Runtime.InteropServices;
using Vortice.MediaFoundation;

namespace ScreenForge.Record;

internal sealed class MfH264Writer : IDisposable
{
    private IMFSinkWriter? _writer;
    private int _streamIndex = -1;
    private int _audioStream = -1;
    private readonly object _writeLock = new();
    private readonly int _width;
    private readonly int _height;
    private readonly long _frameDuration;
    private readonly int _audioRate;
    private bool _started;
    private bool _disposed;
    private bool _mfHeld;

    public MfH264Writer(string path, int width, int height, int fps, int bitrate, bool audio = false, bool hardware = true, int audioSampleRate = 48000)
    {
        _width = width;
        _height = height;
        fps = Math.Max(1, fps);
        _frameDuration = 10_000_000L / fps;
        _audioRate = audioSampleRate is 44100 or 48000 ? audioSampleRate : 48000;

        MfSession.Enter();
        _mfHeld = true;

        try
        {
            using var attrs = MediaFactory.MFCreateAttributes(8);
            attrs.Set(SinkWriterAttributeKeys.ReadwriteEnableHardwareTransforms, hardware);
            // D3D aygıtı yoksa veya meşgulse yazılım dönüşümüne düş; aksi halde
            // sink ilk örnekte kendini Shutdown eder (MF_E_SHUTDOWN).
            attrs.Set(SinkWriterAttributeKeys.ReadwriteD3DOptional, true);
            attrs.Set(SinkWriterAttributeKeys.DisableThrottling, true);

            _writer = MediaFactory.MFCreateSinkWriterFromURL(path, null, attrs);
        }
        catch (Exception ex)
        {
            Dispose();
            throw new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).", ex);
        }

        using (var outType = MediaFactory.MFCreateMediaType())
        {
            outType.Set(MediaTypeAttributeKeys.MajorType, MediaTypeGuids.Video);
            outType.Set(MediaTypeAttributeKeys.Subtype, H264);
            outType.Set(MediaTypeAttributeKeys.AvgBitrate, (uint)bitrate);
            outType.Set(MediaTypeAttributeKeys.InterlaceMode, (uint)VideoInterlaceMode.Progressive);
            outType.Set(MediaTypeAttributeKeys.FrameSize, Pack(width, height));
            outType.Set(MediaTypeAttributeKeys.FrameRate, Pack(fps, 1));
            outType.Set(MediaTypeAttributeKeys.PixelAspectRatio, Pack(1, 1));
            _streamIndex = _writer!.AddStream(outType);
        }

        using (var inType = MediaFactory.MFCreateMediaType())
        {
            inType.Set(MediaTypeAttributeKeys.MajorType, MediaTypeGuids.Video);
            inType.Set(MediaTypeAttributeKeys.Subtype, VideoFormatGuids.Rgb32);
            inType.Set(MediaTypeAttributeKeys.InterlaceMode, (uint)VideoInterlaceMode.Progressive);
            inType.Set(MediaTypeAttributeKeys.FrameSize, Pack(width, height));
            inType.Set(MediaTypeAttributeKeys.FrameRate, Pack(fps, 1));
            inType.Set(MediaTypeAttributeKeys.PixelAspectRatio, Pack(1, 1));
            try
            {
                _writer!.SetInputMediaType(_streamIndex, inType, null);
            }
            catch (Exception ex)
            {
                Dispose();
                throw new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).", ex);
            }
        }

        try
        {
            _audioStream = audio ? TryAddAudio() : -1;
            _writer!.BeginWriting();
            _started = true;
        }
        catch (Exception ex)
        {
            Dispose();
            throw new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).", ex);
        }
    }

    public bool HasAudio => _audioStream >= 0;

    /// <summary>Yazar kapanmışsa kareyi yutar ve false döner. Çağıran durdurma yarışında bunu hata saymasın.</summary>
    public bool WriteBgra(byte[] bgra, long timestampHundredNanos, long durationHundredNanos = 0)
    {
        int stride = _width * 4;
        int byteCount = stride * _height;
        lock (_writeLock)
        {
            if (_disposed || !_started || _writer == null)
                return false;

            using var buffer = MediaFactory.MFCreateMemoryBuffer(byteCount);
            buffer.Lock(out nint data, out _, out _);
            try
            {
                for (int y = 0; y < _height; y++)
                {
                    Marshal.Copy(
                        bgra,
                        y * stride,
                        data + (_height - 1 - y) * stride,
                        stride);
                }
            }
            finally { buffer.Unlock(); }
            buffer.CurrentLength = byteCount;

            using var sample = MediaFactory.MFCreateSample();
            sample.AddBuffer(buffer);
            sample.SampleTime = timestampHundredNanos;
            sample.SampleDuration = durationHundredNanos > 0 ? durationHundredNanos : _frameDuration;
            _writer.WriteSample(_streamIndex, sample);
            return true;
        }
    }

    public bool WriteAudio(byte[] pcm, long timestampHundredNanos)
    {
        if (_audioStream < 0 || pcm.Length < 4) return false;
        lock (_writeLock)
        {
            if (_disposed || !_started || _writer == null)
                return false;

            using var buffer = MediaFactory.MFCreateMemoryBuffer(pcm.Length);
            buffer.Lock(out nint data, out _, out _);
            try { Marshal.Copy(pcm, 0, data, pcm.Length); }
            finally { buffer.Unlock(); }
            buffer.CurrentLength = pcm.Length;

            long duration = pcm.Length * 10_000_000L / (_audioRate * AudioMixer.BytesPerFrame);
            using var sample = MediaFactory.MFCreateSample();
            sample.AddBuffer(buffer);
            sample.SampleTime = timestampHundredNanos;
            sample.SampleDuration = Math.Max(1, duration);
            _writer.WriteSample(_audioStream, sample);
            return true;
        }
    }

    private int TryAddAudio()
    {
        try
        {
            int stream;
            using (var aac = MediaFactory.MFCreateMediaType())
            {
                aac.Set(MediaTypeAttributeKeys.MajorType, MediaTypeGuids.Audio);
                aac.Set(MediaTypeAttributeKeys.Subtype, AudioFormatGuids.Aac);
                aac.Set(MediaTypeAttributeKeys.AudioNumChannels, 2u);
                aac.Set(MediaTypeAttributeKeys.AudioSamplesPerSecond, (uint)_audioRate);
                aac.Set(MediaTypeAttributeKeys.AudioBitsPerSample, 16u);
                aac.Set(MediaTypeAttributeKeys.AudioAvgBytesPerSecond, 16000u);
                aac.Set(MediaTypeAttributeKeys.AacPayloadType, 0u);
                // LC-AAC Level 2. Bazı Windows sürümleri bu alan olmadan
                // AAC MFT'yi kurar, ilk örnekte sink'i Shutdown eder.
                aac.Set(MediaTypeAttributeKeys.AacAudioProfileLevelIndication, 0x29u);
                stream = _writer!.AddStream(aac);
            }

            using (var pcm = MediaFactory.MFCreateMediaType())
            {
                pcm.Set(MediaTypeAttributeKeys.MajorType, MediaTypeGuids.Audio);
                pcm.Set(MediaTypeAttributeKeys.Subtype, AudioFormatGuids.Pcm);
                pcm.Set(MediaTypeAttributeKeys.AudioNumChannels, 2u);
                pcm.Set(MediaTypeAttributeKeys.AudioSamplesPerSecond, (uint)_audioRate);
                pcm.Set(MediaTypeAttributeKeys.AudioBitsPerSample, 16u);
                pcm.Set(MediaTypeAttributeKeys.AudioBlockAlignment, (uint)AudioMixer.BytesPerFrame);
                pcm.Set(MediaTypeAttributeKeys.AudioAvgBytesPerSecond,
                    (uint)(_audioRate * AudioMixer.BytesPerFrame));
                _writer!.SetInputMediaType(stream, pcm, null);
            }

            return stream;
        }
        catch
        {
            return -1;
        }
    }

    public void Finish()
    {
        lock (_writeLock)
        {
            if (!_started || _writer == null) return;
            _started = false;
            _writer.Finalize();
        }
    }

    public void Dispose()
    {
        lock (_writeLock)
        {
            if (_disposed) return;
            _disposed = true;
            try
            {
                if (_started && _writer != null)
                {
                    _started = false;
                    _writer.Finalize();
                }
            }
            catch { /* already finalized or failed */ }
            _writer?.Dispose();
            _writer = null;
            if (_mfHeld)
            {
                _mfHeld = false;
                MfSession.Exit();
            }
        }
    }

    /// <summary>
    /// MFStartup / MFShutdown süreç genelinde sayaçlıdır. Bir yazar kapanırken
    /// sayacı sıfırlamak, aynı anda açılan ikinci yazarı MF_E_SHUTDOWN ile düşürür.
    /// </summary>
    private static class MfSession
    {
        private static readonly object Gate = new();
        private static int _count;

        public static void Enter()
        {
            lock (Gate)
            {
                if (_count == 0)
                    MediaFactory.MFStartup();
                _count++;
            }
        }

        public static void Exit()
        {
            lock (Gate)
            {
                if (_count == 0) return;
                _count--;
                if (_count == 0)
                    MediaFactory.MFShutdown();
            }
        }
    }

    private static ulong Pack(int hi, int lo) => ((ulong)(uint)hi << 32) | (uint)lo;

    private static readonly Guid H264 = new("34363248-0000-0010-8000-00AA00389B71");
}
