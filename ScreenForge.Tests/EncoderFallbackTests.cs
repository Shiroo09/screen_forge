using ScreenForge.Record;

namespace ScreenForge.Tests;

public sealed class EncoderFallbackTests
{
    [Fact]
    public void Plan_WithAudio_TriesSoftwareBeforeDroppingAudio()
    {
        Assert.Equal(
            [
                new EncoderChoice(true, true),
                new EncoderChoice(false, true),
                new EncoderChoice(true, false),
                new EncoderChoice(false, false),
            ],
            EncoderFallback.Plan(wantAudio: true));
    }

    [Fact]
    public void Plan_WithoutAudio_IsHardwareThenSoftware()
    {
        Assert.Equal(
            [new EncoderChoice(true, false), new EncoderChoice(false, false)],
            EncoderFallback.Plan(wantAudio: false));
    }

    [Fact]
    public void Next_VideoFailure_KeepsAudioOnSoftwareEncoder()
    {
        var plan = EncoderFallback.Plan(wantAudio: true);
        Assert.Equal(1, EncoderFallback.Next(plan, index: 0, audioFailed: false));
    }

    [Fact]
    public void Next_AudioFailure_SkipsChoicesThatStillRecordAudio()
    {
        var plan = EncoderFallback.Plan(wantAudio: true);
        Assert.Equal(2, EncoderFallback.Next(plan, index: 0, audioFailed: true));
        Assert.Equal(3, EncoderFallback.Next(plan, index: 2, audioFailed: true));
        Assert.Equal(-1, EncoderFallback.Next(plan, index: 3, audioFailed: true));
    }

    [Fact]
    public void IsShutdown_DetectsHRESULTAndInnerException()
    {
        var inner = new InvalidOperationException("MF_E_SHUTDOWN");
        inner.HResult = EncoderFallback.MfEShutdown;
        var wrapped = new InvalidOperationException("Bu cihazda H.264 kodlayıcı yok (Media Foundation).", inner);

        Assert.True(EncoderFallback.IsShutdown(inner));
        Assert.True(EncoderFallback.IsShutdown(wrapped));
        Assert.False(EncoderFallback.IsShutdown(new InvalidOperationException("disk full")));
    }
}
