using System.Runtime.InteropServices;
using NAudio.Wave;

namespace ScreenForge.Record;

/// <summary>
/// Sistem sesini çıkış aygıtından değil, uygulamaların karışımından alır.
/// Bluetooth kulaklık döngüsü paketleri böldüğü için kayıt titriyordu.
/// </summary>
internal sealed class SystemLoopback : IDisposable
{
    private readonly Handler _handler;
    private readonly ManualResetEventSlim _ready = new(false);
    private IAudioClient? _client;
    private IAudioCaptureClient? _capture;
    private Thread? _thread;
    private Exception? _error;
    private volatile bool _run;
    private int _activateHr;
    private bool _started;

    public SystemLoopback()
    {
        WaveFormat = new WaveFormat(48000, 16, 2);
        _handler = new Handler(this);
    }

    public WaveFormat WaveFormat { get; }

    public event EventHandler<WaveInEventArgs>? DataAvailable;

    public void Start()
    {
        if (_started)
            return;
        _run = true;
        _thread = new Thread(ThreadMain)
        {
            IsBackground = true,
            Name = "ScreenForge system audio",
        };
        _thread.SetApartmentState(ApartmentState.MTA);
        _thread.Start();
        if (!_ready.Wait(5000))
            throw new TimeoutException("Sistem sesi açılamadı.");
        if (_error != null)
            throw _error;
        if (_activateHr < 0)
            Marshal.ThrowExceptionForHR(_activateHr);
        _started = true;
    }

    public void Stop()
    {
        _run = false;
        _thread?.Join(1500);
        try { _client?.Stop(); } catch { /* ignore */ }
    }

    public void Dispose()
    {
        Stop();
        _capture = null;
        _client = null;
        _ready.Dispose();
    }

    private void ThreadMain()
    {
        IntPtr blob = IntPtr.Zero;
        try
        {
            var activate = new AudioClientActivationParams
            {
                ActivationType = 1,
                TargetProcessId = (uint)Environment.ProcessId,
                ProcessLoopbackMode = 1,
            };
            int size = Marshal.SizeOf<AudioClientActivationParams>();
            blob = Marshal.AllocCoTaskMem(size);
            Marshal.StructureToPtr(activate, blob, false);
            var pv = new PropVariantBlob { vt = 65, blobSize = size, blobData = blob };
            Guid iid = typeof(IAudioClient).GUID;
            int hr = NativeMethods.ActivateAudioInterfaceAsync("VAD\\Process_Loopback", ref iid, ref pv, _handler, out _);
            if (hr < 0)
            {
                _activateHr = hr;
                return;
            }

            if (!_ready.Wait(4000) || _capture == null || _activateHr < 0)
                return;

            var buffer = new byte[WaveFormat.AverageBytesPerSecond / 2];
            while (_run)
            {
                Thread.Sleep(8);
                Drain(buffer);
            }

            Drain(buffer);
        }
        catch (Exception ex)
        {
            _error = ex;
        }
        finally
        {
            if (blob != IntPtr.Zero)
                Marshal.FreeCoTaskMem(blob);
            _ready.Set();
        }
    }

    private void Drain(byte[] buffer)
    {
        var cap = _capture;
        if (cap == null)
            return;
        while (_run)
        {
            cap.GetNextPacketSize(out int packet);
            if (packet == 0)
                return;
            cap.GetBuffer(out IntPtr data, out int frames, out int flags, out _, out _);
            try
            {
                int bytes = frames * WaveFormat.BlockAlign;
                if (bytes <= 0)
                    continue;
                if (bytes > buffer.Length)
                    buffer = new byte[bytes];
                if ((flags & 2) != 0 || data == IntPtr.Zero)
                    Array.Clear(buffer, 0, bytes);
                else
                    Marshal.Copy(data, buffer, 0, bytes);
                DataAvailable?.Invoke(this, new WaveInEventArgs(buffer, bytes));
            }
            finally
            {
                cap.ReleaseBuffer(frames);
            }
        }
    }

    private void OnActivated(IActivateAudioInterfaceAsyncOperation operation)
    {
        try
        {
            operation.GetActivateResult(out int hr, out object? unk);
            _activateHr = hr;
            if (hr < 0 || unk is not IAudioClient client)
                return;

            _client = client;
            var wfx = new WaveFormatEx
            {
                wFormatTag = 1,
                nChannels = 2,
                nSamplesPerSec = 48000,
                wBitsPerSample = 16,
                nBlockAlign = 4,
                nAvgBytesPerSec = 48000 * 4,
            };
            client.Initialize(0, 0x00020000 | unchecked((int)0x80000000), 0, 0, ref wfx, IntPtr.Zero);
            Guid capId = typeof(IAudioCaptureClient).GUID;
            client.GetService(ref capId, out object cap);
            _capture = (IAudioCaptureClient)cap;
            client.Start();
        }
        catch (Exception ex)
        {
            _error = ex;
            _activateHr = _activateHr < 0 ? _activateHr : unchecked((int)0x80004005);
        }
        finally
        {
            _ready.Set();
        }
    }

    private sealed class Handler : IActivateAudioInterfaceCompletionHandler
    {
        private readonly SystemLoopback _owner;
        public Handler(SystemLoopback owner) => _owner = owner;
        public void ActivateCompleted(IActivateAudioInterfaceAsyncOperation operation) => _owner.OnActivated(operation);
    }
}

[ComImport, Guid("41D949AB-9862-444A-80F6-C261334DA5EB"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IActivateAudioInterfaceCompletionHandler
{
    void ActivateCompleted(IActivateAudioInterfaceAsyncOperation operation);
}

[ComImport, Guid("72A22D78-CDE4-431D-B8CC-843A71199B6D"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IActivateAudioInterfaceAsyncOperation
{
    void GetActivateResult(out int activateResult, [MarshalAs(UnmanagedType.IUnknown)] out object? activatedInterface);
}

[ComImport, Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioClient
{
    void Initialize(int shareMode, int streamFlags, long bufferDuration, long periodicity, ref WaveFormatEx format, IntPtr sessionGuid);
    void GetBufferSize(out int numBufferFrames);
    void GetStreamLatency(out long latency);
    void GetCurrentPadding(out int numPaddingFrames);
    void IsFormatSupported(int shareMode, IntPtr format, out IntPtr closest);
    void GetMixFormat(out IntPtr deviceFormat);
    void GetDevicePeriod(out long defaultPeriod, out long minimumPeriod);
    void Start();
    void Stop();
    void Reset();
    void SetEventHandle(IntPtr eventHandle);
    void GetService(ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object service);
}

[ComImport, Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioCaptureClient
{
    void GetBuffer(out IntPtr data, out int numFrames, out int flags, out long devicePosition, out long qpcPosition);
    void ReleaseBuffer(int numFrames);
    void GetNextPacketSize(out int numFrames);
}

[StructLayout(LayoutKind.Sequential)]
internal struct WaveFormatEx
{
    public ushort wFormatTag;
    public ushort nChannels;
    public uint nSamplesPerSec;
    public uint nAvgBytesPerSec;
    public ushort nBlockAlign;
    public ushort wBitsPerSample;
    public ushort cbSize;
}

[StructLayout(LayoutKind.Sequential)]
internal struct AudioClientActivationParams
{
    public int ActivationType;
    public uint TargetProcessId;
    public uint ProcessLoopbackMode;
}

[StructLayout(LayoutKind.Explicit, Size = 24)]
internal struct PropVariantBlob
{
    [FieldOffset(0)] public ushort vt;
    [FieldOffset(8)] public int blobSize;
    [FieldOffset(16)] public IntPtr blobData;
}

internal static class NativeMethods
{
    [DllImport("Mmdevapi.dll", ExactSpelling = true, PreserveSig = true)]
    public static extern int ActivateAudioInterfaceAsync(
        [MarshalAs(UnmanagedType.LPWStr)] string deviceInterfacePath,
        [In] ref Guid riid,
        ref PropVariantBlob activationParams,
        [MarshalAs(UnmanagedType.Interface)] IActivateAudioInterfaceCompletionHandler handler,
        out IntPtr operation);
}
