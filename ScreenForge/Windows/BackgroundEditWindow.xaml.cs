using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using ScreenForge.Background;
using ScreenForge.WindowsAI;
using SkiaSharp;

namespace ScreenForge.Windows;

public partial class BackgroundEditWindow : Window
{
    private const int ModelMaxEdge = 1536;

    private readonly string _path;
    private SKBitmap? _full;
    private SKBitmap? _working;
    private byte[]? _mask;
    private SKBitmap? _replacement;
    private BackgroundMode _mode = BackgroundMode.Original;
    private SKColor _color = SKColors.White;
    private int _generation;
    private readonly List<HintPoint> _include = new();
    private readonly List<HintPoint> _exclude = new();
    private readonly List<(Border Frame, SKColor Color)> _swatches = new();

    public BackgroundEditWindow(string path)
    {
        _path = path;
        InitializeComponent();
        FileLabel.Text = System.IO.Path.GetFileName(path);
        SourceInitialized += (_, _) => DarkTitleBar.Apply(this);
        Loaded += async (_, _) => await LoadAsync();

        BtnRemove.Click += (_, _) => SetMode(BackgroundMode.Remove);
        BtnColor.Click += (_, _) => SetMode(BackgroundMode.Color);
        BtnImage.Click += (_, _) => PickReplacement();
        BtnReset.Click += (_, _) => Reset();
        BtnSave.Click += (_, _) => Save(overwrite: true);
        BtnSaveAs.Click += (_, _) => Save(overwrite: false);
        BtnCustomColor.Click += (_, _) => OpenColorPicker();
        HexBox.KeyDown += (_, e) =>
        {
            if (e.Key == Key.Enter)
                CommitHex();
        };
        HexBox.LostFocus += (_, _) => CommitHex();
        Preview.MouseLeftButtonDown += (_, e) => AddHint(e, include: true);
        Preview.MouseRightButtonDown += (_, e) => AddHint(e, include: false);

        AddSwatch(SKColors.White);
        AddSwatch(new SKColor(0xF4, 0xF1, 0xEA));
        AddSwatch(new SKColor(0xE6, 0xE6, 0xE6));
        AddSwatch(new SKColor(0xB8, 0xB8, 0xB8));
        AddSwatch(new SKColor(0x6B, 0x72, 0x80));
        AddSwatch(new SKColor(0x1F, 0x24, 0x30));
        AddSwatch(SKColors.Black);
        AddSwatch(new SKColor(0xEA, 0x6F, 0x12));
        AddSwatch(new SKColor(0xE5, 0x48, 0x4D));
        AddSwatch(new SKColor(0xF5, 0xC1, 0x6C));
        AddSwatch(new SKColor(0x2F, 0x9E, 0x6B));
        AddSwatch(new SKColor(0x1F, 0x6F, 0xEB));
        SyncColorUi();
    }

    private async Task LoadAsync()
    {
        try
        {
            _full = BackgroundFiles.DecodeOriented(_path);
            _working = BackgroundCompositor.Downscale(_full, ModelMaxEdge);
            ShowBitmap(_full);
            await SegmentAsync(centerRectangle: true);
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }

    private async Task SegmentAsync(bool centerRectangle)
    {
        if (_working == null)
            return;

        int generation = ++_generation;
        SetBusy(true);
        Status.Text = "Arka plan ayrılıyor…";
        try
        {
            var segmented = await BackgroundSegmenter.ExtractAsync(
                _working,
                _include,
                _exclude,
                centerRectangle);
            var mask = segmented.Mask;
            if (segmented.UsedLocalModel)
                Hint.Text = "Yerel model kullanılıyor. İşaret tıklamaları bu modelde maskeyi değiştirmez.";
            if (generation != _generation)
                return;
            _mask = mask;
            Status.Text = "";
            if (_mode == BackgroundMode.Original)
                SetMode(BackgroundMode.Remove);
            else
                RenderPreview();
        }
        catch (Exception ex)
        {
            if (generation != _generation)
                return;
            Status.Text = ex.Message;
        }
        finally
        {
            if (generation == _generation)
                SetBusy(false);
        }
    }

    private void SetMode(BackgroundMode mode)
    {
        _mode = mode;
        ColorPanel.Visibility = mode == BackgroundMode.Color ? Visibility.Visible : Visibility.Collapsed;
        PaintModeButton(BtnRemove, mode == BackgroundMode.Remove);
        PaintModeButton(BtnColor, mode == BackgroundMode.Color);
        RenderPreview();
    }

    private void PaintModeButton(Button button, bool selected)
    {
        button.Background = (Brush)FindResource(selected ? "AccentBrush" : "SurfaceAltBrush");
        button.Foreground = selected ? Brushes.White : (Brush)FindResource("TextBrush");
    }

    private void Reset()
    {
        _mode = BackgroundMode.Original;
        _include.Clear();
        _exclude.Clear();
        _replacement?.Dispose();
        _replacement = null;
        ColorPanel.Visibility = Visibility.Collapsed;
        PaintModeButton(BtnRemove, false);
        PaintModeButton(BtnColor, false);
        if (_full != null)
            ShowBitmap(_full);
        _ = SegmentAsync(centerRectangle: true);
    }

    private void RenderPreview()
    {
        if (_full == null)
            return;
        if (_mask == null || _working == null || _mode == BackgroundMode.Original)
        {
            ShowBitmap(_full);
            return;
        }

        var style = new BackgroundStyle(_mode, _color, _replacement);
        using var result = BackgroundCompositor.Apply(_full, _mask, _working.Width, _working.Height, style);
        ShowBitmap(result);
    }

    private void ShowBitmap(SKBitmap bitmap)
    {
        Preview.Source = ToBitmapSource(bitmap);
    }

    private void AddHint(MouseButtonEventArgs e, bool include)
    {
        if (_full == null || _working == null || Preview.Source is not BitmapSource source)
            return;

        var position = e.GetPosition(Preview);
        if (!TryMapToImage(position, source.PixelWidth, source.PixelHeight, out int x, out int y))
            return;

        float sx = _working.Width / (float)_full.Width;
        float sy = _working.Height / (float)_full.Height;
        var hint = new HintPoint(
            Math.Clamp((int)Math.Round(x * sx), 0, _working.Width - 1),
            Math.Clamp((int)Math.Round(y * sy), 0, _working.Height - 1));
        if (include)
            _include.Add(hint);
        else
            _exclude.Add(hint);
        _ = SegmentAsync(centerRectangle: false);
        e.Handled = true;
    }

    private bool TryMapToImage(Point position, int pixelWidth, int pixelHeight, out int x, out int y)
    {
        x = 0;
        y = 0;
        double viewW = Preview.ActualWidth;
        double viewH = Preview.ActualHeight;
        if (viewW <= 0 || viewH <= 0)
            return false;

        double scale = Math.Min(viewW / pixelWidth, viewH / pixelHeight);
        double drawnW = pixelWidth * scale;
        double drawnH = pixelHeight * scale;
        double left = (viewW - drawnW) / 2;
        double top = (viewH - drawnH) / 2;
        if (position.X < left || position.Y < top || position.X >= left + drawnW || position.Y >= top + drawnH)
            return false;

        x = (int)((position.X - left) / scale);
        y = (int)((position.Y - top) / scale);
        x = Math.Clamp(x, 0, pixelWidth - 1);
        y = Math.Clamp(y, 0, pixelHeight - 1);
        return true;
    }

    private void PickReplacement()
    {
        var dialog = new Microsoft.Win32.OpenFileDialog
        {
            Filter = "Görüntüler|*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.gif;*.tif;*.tiff",
            Title = "Yeni arka plan",
        };
        if (dialog.ShowDialog(this) != true)
            return;

        try
        {
            _replacement?.Dispose();
            _replacement = BackgroundFiles.DecodeOriented(dialog.FileName);
            SetMode(BackgroundMode.Image);
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }

    private void ColorWell_Click(object sender, MouseButtonEventArgs e)
    {
        OpenColorPicker();
        e.Handled = true;
    }

    private void OpenColorPicker()
    {
        var initial = Color.FromRgb(_color.Red, _color.Green, _color.Blue);
        var popup = new ColorPickerPopup(ColorWell, initial, c => ApplyColor(new SKColor(c.R, c.G, c.B)));
        popup.Open();
    }

    private void CommitHex()
    {
        var text = HexBox.Text.Trim().TrimStart('#');
        if (text.Length == 6 &&
            uint.TryParse(text, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out uint rgb))
        {
            ApplyColor(new SKColor((byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb));
            return;
        }

        SyncColorUi();
    }

    private void ApplyColor(SKColor color)
    {
        _color = new SKColor(color.Red, color.Green, color.Blue);
        SyncColorUi();
        SetMode(BackgroundMode.Color);
    }

    private void SyncColorUi()
    {
        ColorWell.Background = new SolidColorBrush(Color.FromRgb(_color.Red, _color.Green, _color.Blue));
        if (!HexBox.IsKeyboardFocusWithin)
            HexBox.Text = $"#{_color.Red:X2}{_color.Green:X2}{_color.Blue:X2}";

        var accent = (Brush)FindResource("AccentBrush");
        var border = (Brush)FindResource("BorderBrush");
        foreach (var (frame, color) in _swatches)
        {
            bool on = color.Red == _color.Red && color.Green == _color.Green && color.Blue == _color.Blue;
            frame.BorderBrush = on ? accent : border;
            frame.BorderThickness = new Thickness(on ? 2 : 1);
        }
    }

    private void AddSwatch(SKColor color)
    {
        var chip = new Border
        {
            Width = 22,
            Height = 22,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(Color.FromRgb(color.Red, color.Green, color.Blue)),
        };
        var frame = new Border
        {
            Width = 32,
            Height = 32,
            Margin = new Thickness(0, 0, 6, 6),
            CornerRadius = new CornerRadius(7),
            BorderBrush = (Brush)FindResource("BorderBrush"),
            BorderThickness = new Thickness(1),
            Background = Brushes.Transparent,
            Cursor = Cursors.Hand,
            Child = chip,
            Padding = new Thickness(3),
            ToolTip = $"#{color.Red:X2}{color.Green:X2}{color.Blue:X2}",
        };
        frame.MouseLeftButtonUp += (_, e) =>
        {
            ApplyColor(color);
            e.Handled = true;
        };
        _swatches.Add((frame, color));
        Swatches.Children.Add(frame);
    }

    private void Save(bool overwrite)
    {
        if (_full == null)
            return;

        SKBitmap? rendered = null;
        try
        {
            if (_mask != null && _working != null && _mode != BackgroundMode.Original)
            {
                rendered = BackgroundCompositor.Apply(
                    _full,
                    _mask,
                    _working.Width,
                    _working.Height,
                    new BackgroundStyle(_mode, _color, _replacement));
            }

            var bitmap = rendered ?? _full;
            bool transparent = BackgroundCompositor.HasTransparency(bitmap);
            string path;
            ScreenForge.Settings.ImageFormat format;
            if (overwrite)
            {
                var plan = BackgroundFiles.PlanOverwrite(_path, transparent);
                path = plan.Path;
                format = plan.Format;
                if (!plan.ReplacesOriginal)
                    Status.Text = "Saydamlık JPEG'e yazılamaz. PNG olarak kaydedildi: " + System.IO.Path.GetFileName(path);
            }
            else
            {
                var dialog = new Microsoft.Win32.SaveFileDialog
                {
                    Filter = "PNG|*.png|JPEG|*.jpg|WebP|*.webp",
                    FileName = System.IO.Path.GetFileNameWithoutExtension(_path) + "-arka-plan",
                    DefaultExt = transparent ? ".png" : System.IO.Path.GetExtension(_path),
                    AddExtension = true,
                    Title = "Farklı kaydet",
                };
                if (dialog.ShowDialog(this) != true)
                    return;
                path = dialog.FileName;
                format = System.IO.Path.GetExtension(path).ToLowerInvariant() switch
                {
                    ".jpg" or ".jpeg" => ScreenForge.Settings.ImageFormat.Jpeg,
                    ".webp" => ScreenForge.Settings.ImageFormat.Webp,
                    _ => ScreenForge.Settings.ImageFormat.Png,
                };
            }

            BackgroundFiles.Write(bitmap, path, format);
            if (overwrite && Status.Text.Length == 0)
                Status.Text = "Kaydedildi.";
            if (!overwrite)
                Status.Text = "Kaydedildi.";
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
        finally
        {
            rendered?.Dispose();
        }
    }

    private void SetBusy(bool busy)
    {
        BtnRemove.IsEnabled = !busy;
        BtnColor.IsEnabled = !busy;
        BtnImage.IsEnabled = !busy;
        BtnReset.IsEnabled = !busy;
        BtnSave.IsEnabled = !busy;
        BtnSaveAs.IsEnabled = !busy;
    }

    private static BitmapSource ToBitmapSource(SKBitmap bitmap)
    {
        var pixels = new byte[bitmap.RowBytes * bitmap.Height];
        Marshal.Copy(bitmap.GetPixels(), pixels, 0, pixels.Length);
        var source = BitmapSource.Create(
            bitmap.Width,
            bitmap.Height,
            96,
            96,
            PixelFormats.Bgra32,
            null,
            pixels,
            bitmap.RowBytes);
        source.Freeze();
        return source;
    }

    protected override void OnClosed(EventArgs e)
    {
        _generation++;
        _full?.Dispose();
        _working?.Dispose();
        _replacement?.Dispose();
        base.OnClosed(e);
    }
}
