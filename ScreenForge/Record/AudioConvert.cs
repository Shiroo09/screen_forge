using NAudio.Wave;

namespace ScreenForge.Record;

/// <summary>
/// Yakalanan sesi stereo float'a çevirir. Hedef hız kaynakla aynıysa örneklemez.
/// 44.1 kHz döngü sesini 48 kHz'e çekmek her pakette fazı bozup sesi taktırıyordu.
/// </summary>
internal sealed class PcmPump
{
    private readonly WaveFormat _format;
    private readonly int _targetRate;
    private readonly bool _passthrough;
    private readonly double _step;
    private float[] _pending = [];
    private double _phase;

    public int Target => _targetRate;

    public PcmPump(WaveFormat format, int targetRate)
    {
        _format = format;
        _targetRate = targetRate > 0 ? targetRate : 48000;
        _passthrough = format.SampleRate == _targetRate;
        _step = format.SampleRate / (double)Math.Max(1, _targetRate);
    }

    public float[] Push(byte[] buffer, int count, out float peak)
    {
        float[] src = AudioConvert.ToStereo(buffer, count, _format, out peak);
        if (_passthrough || src.Length == 0)
            return src;
        return Resample(src, ref peak);
    }

    private float[] Resample(float[] src, ref float peak)
    {
        if (_pending.Length == 0)
        {
            _pending = src;
        }
        else
        {
            var merged = new float[_pending.Length + src.Length];
            _pending.CopyTo(merged, 0);
            src.CopyTo(merged, _pending.Length);
            _pending = merged;
        }

        int frames = _pending.Length / 2;
        var acc = new List<float>(src.Length + 8);
        while (_phase + 1 < frames)
        {
            int i = (int)_phase;
            float t = (float)(_phase - i);
            int a = i * 2;
            int b = a + 2;
            float l = _pending[a] + (_pending[b] - _pending[a]) * t;
            float r = _pending[a + 1] + (_pending[b + 1] - _pending[a + 1]) * t;
            acc.Add(l);
            acc.Add(r);
            peak = Math.Max(peak, Math.Max(Math.Abs(l), Math.Abs(r)));
            _phase += _step;
        }

        int drop = (int)_phase;
        if (drop > 0)
        {
            int remain = frames - drop;
            if (remain <= 0)
            {
                _pending = [];
                _phase = 0;
            }
            else
            {
                _pending = _pending.AsSpan(drop * 2).ToArray();
                _phase -= drop;
            }
        }

        return acc.ToArray();
    }
}

internal static class AudioConvert
{
    public static float[] ToStereo(byte[] buffer, int bytes, WaveFormat format, out float peak)
    {
        peak = 0;
        if (bytes <= 0 || format.SampleRate <= 0 || format.Channels <= 0)
            return [];

        int block = format.BlockAlign > 0 ? format.BlockAlign : format.Channels * Math.Max(format.BitsPerSample, 8) / 8;
        if (block <= 0)
            return [];

        int srcFrames = bytes / block;
        if (srcFrames <= 0)
            return [];

        int channels = format.Channels;
        var src = new float[srcFrames * 2];
        bool isFloat = IsFloat(format);
        int sampleStride = Math.Max(1, block / channels);
        for (int i = 0; i < srcFrames; i++)
        {
            int baseByte = i * block;
            float l = SampleAt(buffer, baseByte, format, isFloat);
            float r = channels > 1
                ? SampleAt(buffer, baseByte + sampleStride, format, isFloat)
                : l;
            src[i * 2] = l;
            src[i * 2 + 1] = r;
            peak = Math.Max(peak, Math.Max(Math.Abs(l), Math.Abs(r)));
        }

        return src;
    }

    private static bool IsFloat(WaveFormat format)
    {
        if (format.Encoding == WaveFormatEncoding.IeeeFloat)
            return true;
        return format is WaveFormatExtensible ext
            && ext.SubFormat == new Guid("00000003-0000-0010-8000-00AA00389B71");
    }

    private static float SampleAt(byte[] buffer, int offset, WaveFormat format, bool isFloat)
    {
        if (offset < 0 || offset >= buffer.Length)
            return 0;
        if (isFloat)
        {
            if (offset + 4 > buffer.Length)
                return 0;
            return BitConverter.ToSingle(buffer, offset);
        }

        if (format.BitsPerSample == 16)
        {
            if (offset + 2 > buffer.Length)
                return 0;
            return BitConverter.ToInt16(buffer, offset) / 32768f;
        }

        if (format.BitsPerSample == 32 && offset + 4 <= buffer.Length)
            return BitConverter.ToInt32(buffer, offset) / 2147483648f;

        return 0;
    }
}
