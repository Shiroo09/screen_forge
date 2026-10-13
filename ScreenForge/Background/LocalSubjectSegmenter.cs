using System.IO;
using SkiaSharp;
using Windows.AI.MachineLearning;

namespace ScreenForge.Background;

/// <summary>
/// U²-Net küçük modeli (u2netp, Apache-2.0, Xuebin Qin). Windows AI kapalıyken
/// tek tıkla konu maskesi üretir. Ağırlıklar rembg v0.0.0 dağıtımındaki onnx dosyasıdır.
/// </summary>
public static class LocalSubjectSegmenter
{
    public const int InputSize = 320;

    static readonly object Gate = new();
    static LearningModelSession? _session;
    static string? _inputName;
    static string? _outputName;

    public static Task<byte[]> ExtractAsync(SKBitmap bitmap, CancellationToken cancellationToken = default)
    {
        return Task.Run(() => Extract(bitmap), cancellationToken);
    }

    public static byte[] Extract(SKBitmap bitmap)
    {
        if (bitmap.Width <= 0 || bitmap.Height <= 0)
            throw new ArgumentOutOfRangeException(nameof(bitmap));

        var input = PackInput(bitmap);
        var tensor = TensorFloat.CreateFromArray(new long[] { 1, 3, InputSize, InputSize }, input);
        LearningModelSession session;
        string inputName;
        string outputName;
        lock (Gate)
        {
            session = Session;
            inputName = _inputName!;
            outputName = _outputName!;
        }

        var binding = new LearningModelBinding(session);
        binding.Bind(inputName, tensor);
        LearningModelEvaluationResult result;
        lock (Gate)
            result = session.Evaluate(binding, "u2netp");

        if (result.Outputs[outputName] is not TensorFloat output)
            throw new InvalidOperationException("Yerel model maske döndürmedi.");

        var view = output.GetAsVectorView();
        int plane = InputSize * InputSize;
        if (view.Count < plane)
            throw new InvalidOperationException("Yerel model maskesi beklenen boyutta değil.");

        var raw = new float[plane];
        for (int i = 0; i < plane; i++)
            raw[i] = view[i];

        return ResizeMask(raw, InputSize, InputSize, bitmap.Width, bitmap.Height);
    }

    private static LearningModelSession Session
    {
        get
        {
            if (_session != null)
                return _session;

            string path = Path.Combine(AppContext.BaseDirectory, "u2netp.onnx");
            if (!File.Exists(path))
                throw new FileNotFoundException("Yerel arka plan modeli bulunamadı.", path);

            var model = LearningModel.LoadFromFilePath(path);
            _inputName = model.InputFeatures[0].Name;
            _outputName = model.OutputFeatures[0].Name;
            _session = new LearningModelSession(model, new LearningModelDevice(LearningModelDeviceKind.Cpu));
            return _session;
        }
    }

    internal static float[] PackInput(SKBitmap source)
    {
        using var scaled = source.Resize(
            new SKImageInfo(InputSize, InputSize, SKColorType.Bgra8888, SKAlphaType.Unpremul),
            new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.None))
            ?? throw new InvalidOperationException("Görüntü modele sığdırılamadı.");

        var pixels = scaled.GetPixelSpan();
        int row = scaled.RowBytes;
        int max = 1;
        for (int y = 0; y < InputSize; y++)
        {
            int o = y * row;
            for (int x = 0; x < InputSize; x++)
            {
                int i = o + x * 4;
                if (pixels[i] > max) max = pixels[i];
                if (pixels[i + 1] > max) max = pixels[i + 1];
                if (pixels[i + 2] > max) max = pixels[i + 2];
            }
        }

        var data = new float[3 * InputSize * InputSize];
        float inv = 1f / max;
        int plane = InputSize * InputSize;
        for (int y = 0; y < InputSize; y++)
        {
            int o = y * row;
            for (int x = 0; x < InputSize; x++)
            {
                int i = o + x * 4;
                int p = y * InputSize + x;
                float b = pixels[i] * inv;
                float g = pixels[i + 1] * inv;
                float r = pixels[i + 2] * inv;
                data[p] = (r - 0.485f) / 0.229f;
                data[plane + p] = (g - 0.456f) / 0.224f;
                data[plane * 2 + p] = (b - 0.406f) / 0.225f;
            }
        }

        return data;
    }

    internal static byte[] ResizeMask(float[] src, int sw, int sh, int dw, int dh)
    {
        int n = sw * sh;
        float min = src[0];
        float max = src[0];
        for (int i = 1; i < n; i++)
        {
            if (src[i] < min) min = src[i];
            if (src[i] > max) max = src[i];
        }

        float span = max - min;
        if (span < 1e-6f)
            span = 1f;

        var dst = new byte[dw * dh];
        for (int y = 0; y < dh; y++)
        {
            float fy = (y + 0.5f) * sh / dh - 0.5f;
            int y0 = (int)Math.Floor(fy);
            float ty = fy - y0;
            if (y0 < 0) { y0 = 0; ty = 0; }
            int y1 = y0 + 1;
            if (y1 >= sh) y1 = sh - 1;

            for (int x = 0; x < dw; x++)
            {
                float fx = (x + 0.5f) * sw / dw - 0.5f;
                int x0 = (int)Math.Floor(fx);
                float tx = fx - x0;
                if (x0 < 0) { x0 = 0; tx = 0; }
                int x1 = x0 + 1;
                if (x1 >= sw) x1 = sw - 1;

                float v =
                    src[y0 * sw + x0] * (1 - tx) * (1 - ty) +
                    src[y0 * sw + x1] * tx * (1 - ty) +
                    src[y1 * sw + x0] * (1 - tx) * ty +
                    src[y1 * sw + x1] * tx * ty;
                float norm = (v - min) / span;
                if (norm < 0) norm = 0;
                if (norm > 1) norm = 1;
                dst[y * dw + x] = (byte)Math.Round(norm * 255f);
            }
        }

        return dst;
    }
}
