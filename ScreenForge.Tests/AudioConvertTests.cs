using NAudio.Wave;
using ScreenForge.Record;

namespace ScreenForge.Tests;

public sealed class AudioConvertTests
{
    [Fact]
    public void SameRate_KeepsDuration()
    {
        var format = new WaveFormat(48000, 16, 2);
        int frames = 480; // 10 ms
        var pcm = new byte[frames * format.BlockAlign];
        float[] samples = AudioConvert.ToStereo(pcm, pcm.Length, format, out _);
        Assert.Equal(frames * 2, samples.Length);
    }

    [Fact]
    public void Native44100_IsNotResampled()
    {
        var format = WaveFormat.CreateIeeeFloatWaveFormat(44100, 2);
        int frames = 441;
        var pcm = new byte[frames * format.BlockAlign];
        var pump = new PcmPump(format, 44100);
        int dst = pump.Push(pcm, pcm.Length, out _).Length / 2;
        Assert.Equal(frames, dst);
    }

    [Fact]
    public void Resample_CarriesPhaseAcrossPackets()
    {
        var format = WaveFormat.CreateIeeeFloatWaveFormat(44100, 2);
        var pump = new PcmPump(format, 48000);
        int frames = 100;
        var pcm = new byte[frames * format.BlockAlign];
        int total = 0;
        for (int i = 0; i < 20; i++)
            total += pump.Push(pcm, pcm.Length, out _).Length / 2;

        // 2000 * 48000 / 44100 = 2176.8. Paket başı sıfırlanan örnekleyici 2160'ta kalır.
        Assert.InRange(total, 2168, 2180);
    }
}
