using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using ScreenForge.Settings;
using IoPath = System.IO.Path;

namespace ScreenForge.Windows;

internal sealed class VideoResultWindow : Window
{
    private readonly string _path;
    private readonly AppSettings _settings;
    private readonly MediaElement _media;
    private readonly Slider _seek;
    private readonly Slider _volume;
    private readonly TextBlock _clock;
    private readonly Button _play;
    private readonly Border _stage;
    private readonly DispatcherTimer _timer;
    private readonly TextBlock _status;
    private bool _clockUpdate;
    private bool _savedAs;
    private bool _deleted;
    private bool _desktopSaved;
    private bool _playerReleased;

    public VideoResultWindow(string path, AppSettings settings)
    {
        _path = path;
        _settings = settings;

        Title = "ScreenForge";
        Width = 780;
        Height = 560;
        MinWidth = 520;
        MinHeight = 400;
        WindowStyle = WindowStyle.None;
        ResizeMode = ResizeMode.CanResize;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = new SolidColorBrush(Color.FromRgb(0x1F, 0x24, 0x30));
        UseLayoutRounding = true;
        SnapsToDevicePixels = true;
        ShowInTaskbar = true;

        var title = new TextBlock
        {
            Text = "Kayıt",
            Foreground = Brushes.White,
            FontSize = 14,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var close = IconButton("\uE8BB");
        close.Click += (_, _) => Close();
        var titleBar = new DockPanel { Margin = new Thickness(4, 0, 0, 8) };
        DockPanel.SetDock(close, Dock.Right);
        titleBar.Children.Add(close);
        titleBar.Children.Add(title);
        titleBar.MouseLeftButtonDown += (_, e) =>
        {
            if (e.ButtonState == MouseButtonState.Pressed)
                DragMove();
        };

        _media = new MediaElement
        {
            LoadedBehavior = MediaState.Manual,
            UnloadedBehavior = MediaState.Manual,
            ScrubbingEnabled = false,
            Stretch = Stretch.Uniform,
            Volume = 0.85,
        };
        _media.MediaOpened += (_, _) => OnMediaOpened();
        _media.MediaEnded += (_, _) =>
        {
            _media.Pause();
            SetPlayIcon(playing: false);
        };
        _media.MediaFailed += (_, e) =>
        {
            if (_status != null)
                _status.Text = "Önizleme açılamadı: " + (e.ErrorException?.Message ?? "");
        };

        _stage = new Border
        {
            Background = Brushes.Black,
            CornerRadius = new CornerRadius(8),
            ClipToBounds = true,
            Child = _media,
        };

        _play = TransportButton("\uE768");
        _play.Click += (_, _) => TogglePlay();
        var stop = TransportButton("\uE71A");
        stop.Click += (_, _) =>
        {
            _media.Pause();
            _media.Position = TimeSpan.Zero;
            SetPlayIcon(playing: false);
            SyncSeek(TimeSpan.Zero);
        };
        var back = TransportButton("\uE100");
        back.ToolTip = "5 sn geri";
        back.Click += (_, _) => Nudge(-5);
        var forward = TransportButton("\uE101");
        forward.ToolTip = "5 sn ileri";
        forward.Click += (_, _) => Nudge(5);

        var sliderStyle = Application.Current?.TryFindResource("ToolSlider") as Style;
        _seek = new Slider
        {
            Minimum = 0,
            Maximum = 1,
            Height = 22,
            Margin = new Thickness(0, 2, 0, 0),
            VerticalAlignment = VerticalAlignment.Center,
            IsMoveToPointEnabled = true,
            Style = sliderStyle,
            FocusVisualStyle = null,
        };
        _seek.ValueChanged += (_, _) =>
        {
            if (_clockUpdate || !_media.NaturalDuration.HasTimeSpan)
                return;
            _media.Position = TimeSpan.FromSeconds(_seek.Value);
            UpdateClock();
        };

        _clock = new TextBlock
        {
            Text = "0:00 / 0:00",
            Foreground = new SolidColorBrush(Color.FromRgb(0xF2, 0xF4, 0xF8)),
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 10, 0),
            MinWidth = 92,
        };

        var volIcon = new TextBlock
        {
            Text = "\uE767",
            FontFamily = new System.Windows.Media.FontFamily("Segoe MDL2 Assets"),
            Foreground = new SolidColorBrush(Color.FromRgb(0x9A, 0xA4, 0xB8)),
            FontSize = 14,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 6, 0),
        };
        _volume = new Slider
        {
            Width = 84,
            Minimum = 0,
            Maximum = 1,
            Value = _media.Volume,
            VerticalAlignment = VerticalAlignment.Center,
            IsMoveToPointEnabled = true,
            Style = sliderStyle,
            FocusVisualStyle = null,
        };
        _volume.ValueChanged += (_, _) => _media.Volume = _volume.Value;

        var buttonsRow = new DockPanel { Margin = new Thickness(0, 6, 0, 8), LastChildFill = false };
        var left = new StackPanel { Orientation = Orientation.Horizontal };
        left.Children.Add(_play);
        left.Children.Add(stop);
        left.Children.Add(back);
        left.Children.Add(forward);
        DockPanel.SetDock(left, Dock.Left);
        var right = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
        right.Children.Add(_clock);
        right.Children.Add(volIcon);
        right.Children.Add(_volume);
        DockPanel.SetDock(right, Dock.Right);
        buttonsRow.Children.Add(left);
        buttonsRow.Children.Add(right);

        var transport = new DockPanel { Margin = new Thickness(0, 10, 0, 0), LastChildFill = true };
        DockPanel.SetDock(_seek, Dock.Top);
        transport.Children.Add(_seek);
        transport.Children.Add(buttonsRow);

        _status = new TextBlock
        {
            Text = "Kapatırsan kayıt masaüstüne kaydedilir.",
            Foreground = new SolidColorBrush(Color.FromRgb(0x9A, 0xA4, 0xB8)),
            FontSize = 12,
            Margin = new Thickness(0, 0, 0, 10),
        };

        var save = new Button
        {
            Content = "Kaydet…",
            MinHeight = 36,
            Padding = new Thickness(14, 8, 14, 8),
            Margin = new Thickness(0, 0, 8, 0),
            Cursor = Cursors.Hand,
            Style = TryFindResource("PrimaryButton") as Style,
        };
        save.Click += (_, _) => SaveAs();

        var delete = new Button
        {
            Content = "Sil",
            MinHeight = 36,
            Padding = new Thickness(14, 8, 14, 8),
            Cursor = Cursors.Hand,
            Style = TryFindResource("SecondaryButton") as Style,
        };
        if (TryFindResource("DangerBrush") is Brush danger)
            delete.Foreground = danger;
        delete.Click += (_, _) => DeleteRecording();

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Left };
        buttons.Children.Add(save);
        buttons.Children.Add(delete);

        var root = new DockPanel { Margin = new Thickness(14) };
        DockPanel.SetDock(titleBar, Dock.Top);
        DockPanel.SetDock(buttons, Dock.Bottom);
        DockPanel.SetDock(_status, Dock.Bottom);
        DockPanel.SetDock(transport, Dock.Bottom);
        root.Children.Add(titleBar);
        root.Children.Add(buttons);
        root.Children.Add(_status);
        root.Children.Add(transport);
        root.Children.Add(_stage);
        Content = root;

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(200) };
        _timer.Tick += (_, _) =>
        {
            if (!_media.NaturalDuration.HasTimeSpan)
                return;
            _clockUpdate = true;
            _seek.Value = _media.Position.TotalSeconds;
            _clockUpdate = false;
            UpdateClock();
        };

        SourceInitialized += (_, _) => DarkTitleBar.Apply(this);
        Loaded += (_, _) =>
        {
            _media.Source = new Uri(_path);
            _media.Play();
            SetPlayIcon(playing: true);
            _timer.Start();
            Focus();
        };
        KeyDown += OnKey;
    }

    private void OnMediaOpened()
    {
        if (_media.NaturalDuration.HasTimeSpan)
        {
            _seek.Maximum = Math.Max(0.1, _media.NaturalDuration.TimeSpan.TotalSeconds);
            UpdateClock();
        }

        Dispatcher.BeginInvoke(FitToVideo, DispatcherPriority.Loaded);
    }

    private void FitToVideo()
    {
        double vw = _media.NaturalVideoWidth;
        double vh = _media.NaturalVideoHeight;
        if (vw < 2 || vh < 2 || _stage == null)
            return;

        UpdateLayout();
        double chromeW = ActualWidth - _stage.ActualWidth;
        double chromeH = ActualHeight - _stage.ActualHeight;
        if (chromeW < 8 || chromeH < 40 || _stage.ActualWidth < 2)
        {
            chromeW = 28;
            chromeH = 196;
        }

        var work = SystemParameters.WorkArea;
        double maxW = Math.Max(320, work.Width - 40 - chromeW);
        double maxH = Math.Max(180, work.Height - 40 - chromeH);
        double scale = Math.Min(maxW / vw, maxH / vh);
        scale = Math.Min(scale, 1);
        double stageW = vw * scale;
        double stageH = vh * scale;
        // Kontroller sığsın diye çok dar kaydı biraz büyüt. Oran bozulmasın.
        if (stageW < 460 && 460 / vw * vh <= maxH)
        {
            stageW = 460;
            stageH = stageW * vh / vw;
        }

        if (stageH > maxH)
        {
            stageH = maxH;
            stageW = stageH * vw / vh;
        }

        Width = Math.Max(360, stageW + chromeW);
        Height = Math.Max(280, stageH + chromeH);
        MinWidth = Math.Min(420, Width);
        MinHeight = Math.Min(300, Height);
        Left = work.Left + Math.Max(0, (work.Width - Width) / 2);
        Top = work.Top + Math.Max(0, (work.Height - Height) / 2);
    }

    private void TogglePlay()
    {
        if (_play.Tag as string == "play")
        {
            _media.Play();
            SetPlayIcon(playing: true);
            return;
        }

        _media.Pause();
        SetPlayIcon(playing: false);
    }

    private void SetPlayIcon(bool playing)
    {
        _play.Content = playing ? "\uE769" : "\uE768";
        _play.Tag = playing ? "pause" : "play";
        _play.ToolTip = playing ? "Duraklat" : "Oynat";
    }

    private void Nudge(int seconds)
    {
        var next = _media.Position + TimeSpan.FromSeconds(seconds);
        if (next < TimeSpan.Zero)
            next = TimeSpan.Zero;
        if (_media.NaturalDuration.HasTimeSpan && next > _media.NaturalDuration.TimeSpan)
            next = _media.NaturalDuration.TimeSpan;
        _media.Position = next;
        SyncSeek(next);
    }

    private void SyncSeek(TimeSpan position)
    {
        _clockUpdate = true;
        _seek.Value = position.TotalSeconds;
        _clockUpdate = false;
        UpdateClock();
    }

    private void UpdateClock()
    {
        var duration = _media.NaturalDuration.HasTimeSpan ? _media.NaturalDuration.TimeSpan : TimeSpan.Zero;
        _clock.Text = Clock(_media.Position) + " / " + Clock(duration);
    }

    private static string Clock(TimeSpan time)
    {
        if (time < TimeSpan.Zero)
            time = TimeSpan.Zero;
        return time.TotalHours >= 1 ? time.ToString(@"h\:mm\:ss") : time.ToString(@"m\:ss");
    }

    private void OnKey(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            Close();
            e.Handled = true;
        }
        else if (e.Key == Key.Space)
        {
            TogglePlay();
            e.Handled = true;
        }
        else if (e.Key == Key.Left)
        {
            Nudge(-5);
            e.Handled = true;
        }
        else if (e.Key == Key.Right)
        {
            Nudge(5);
            e.Handled = true;
        }
    }

    private void SaveAs()
    {
        var dlg = new Microsoft.Win32.SaveFileDialog
        {
            Filter = "MP4 video|*.mp4",
            InitialDirectory = _settings.SaveDirectory,
            FileName = IoPath.GetFileName(_path),
        };
        if (dlg.ShowDialog(this) != true)
            return;

        ReleasePlayer();
        try
        {
            CopyOver(dlg.FileName);
            _settings.SaveDirectory = IoPath.GetDirectoryName(dlg.FileName) ?? _settings.SaveDirectory;
            _settings.Save();
            _savedAs = true;
            Close();
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, "Kaydedilemedi: " + ex.Message, "ScreenForge",
                MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void DeleteRecording()
    {
        var answer = MessageBox.Show(this, "Bu kayıt silinsin mi?", "ScreenForge",
            MessageBoxButton.YesNo, MessageBoxImage.Warning);
        if (answer != MessageBoxResult.Yes)
            return;
        _deleted = true;
        Close();
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        ReleasePlayer();
        if (_deleted)
        {
            TryDelete(_path);
            base.OnClosing(e);
            return;
        }

        if (!_savedAs && !_desktopSaved)
        {
            try
            {
                SaveToDesktop();
                _desktopSaved = true;
            }
            catch (Exception ex)
            {
                e.Cancel = true;
                MessageBox.Show(this, "Masaüstüne kaydedilemedi: " + ex.Message, "ScreenForge",
                    MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }
        }

        TryDelete(_path);
        base.OnClosing(e);
    }

    private void SaveToDesktop()
    {
        string desktop = Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory);
        Directory.CreateDirectory(desktop);
        string name = IoPath.GetFileName(_path);
        string dest = IoPath.Combine(desktop, name);
        int n = 2;
        while (File.Exists(dest))
        {
            dest = IoPath.Combine(desktop, IoPath.GetFileNameWithoutExtension(name) + "-" + n + ".mp4");
            n++;
        }

        CopyOver(dest);
    }

    private void CopyOver(string dest)
    {
        if (string.Equals(dest, _path, StringComparison.OrdinalIgnoreCase))
            return;
        if (File.Exists(dest))
            File.Delete(dest);
        File.Copy(_path, dest, overwrite: true);
    }

    private void ReleasePlayer()
    {
        if (_playerReleased)
            return;
        _playerReleased = true;
        _timer.Stop();
        _media.Stop();
        _media.Close();
        _media.Source = null;
        // MediaElement dosyayı bırakana kadar kopyalama ve silme başarısız olur.
        GC.Collect();
        GC.WaitForPendingFinalizers();
    }

    private static Button TransportButton(string glyph)
    {
        var button = IconButton(glyph);
        button.Width = 36;
        button.Height = 36;
        button.Margin = new Thickness(0, 0, 4, 0);
        return button;
    }

    private static Button IconButton(string glyph)
    {
        return new Button
        {
            Content = glyph,
            FontFamily = new System.Windows.Media.FontFamily("Segoe MDL2 Assets"),
            FontSize = 14,
            Width = 32,
            Height = 32,
            FocusVisualStyle = null,
            Style = IconButtonStyle,
        };
    }

    private static readonly Style IconButtonStyle = BuildIconStyle();

    private static Style BuildIconStyle()
    {
        var idle = new SolidColorBrush(Color.FromRgb(0x27, 0x2D, 0x3B));
        var hover = new SolidColorBrush(Color.FromRgb(0x32, 0x3A, 0x4C));
        var pressed = new SolidColorBrush(Color.FromRgb(0x1A, 0x20, 0x2C));
        var border = new SolidColorBrush(Color.FromRgb(0x3A, 0x42, 0x54));
        idle.Freeze();
        hover.Freeze();
        pressed.Freeze();
        border.Freeze();

        var shell = new FrameworkElementFactory(typeof(Border));
        shell.SetValue(Border.CornerRadiusProperty, new CornerRadius(6));
        shell.SetValue(Border.BorderBrushProperty, border);
        shell.SetValue(Border.BorderThicknessProperty, new Thickness(1));
        shell.SetBinding(Border.BackgroundProperty, new System.Windows.Data.Binding(nameof(System.Windows.Controls.Control.Background))
        {
            RelativeSource = new System.Windows.Data.RelativeSource(System.Windows.Data.RelativeSourceMode.TemplatedParent),
        });
        var presenter = new FrameworkElementFactory(typeof(ContentPresenter));
        presenter.SetValue(ContentPresenter.HorizontalAlignmentProperty, HorizontalAlignment.Center);
        presenter.SetValue(ContentPresenter.VerticalAlignmentProperty, VerticalAlignment.Center);
        shell.AppendChild(presenter);

        var template = new ControlTemplate(typeof(Button)) { VisualTree = shell };
        template.Triggers.Add(ColorTrigger(UIElement.IsMouseOverProperty, hover));
        template.Triggers.Add(ColorTrigger(Button.IsPressedProperty, pressed));

        var style = new Style(typeof(Button));
        style.Setters.Add(new Setter(System.Windows.Controls.Control.TemplateProperty, template));
        style.Setters.Add(new Setter(System.Windows.Controls.Control.BackgroundProperty, idle));
        style.Setters.Add(new Setter(System.Windows.Controls.Control.ForegroundProperty, Brushes.White));
        style.Setters.Add(new Setter(FrameworkElement.CursorProperty, Cursors.Hand));
        style.Setters.Add(new Setter(System.Windows.Controls.Control.FocusVisualStyleProperty, null));
        return style;
    }

    private static Trigger ColorTrigger(DependencyProperty property, Brush brush)
    {
        var trigger = new Trigger { Property = property, Value = true };
        trigger.Setters.Add(new Setter(System.Windows.Controls.Control.BackgroundProperty, brush));
        return trigger;
    }

    private static void TryDelete(string path)
    {
        try { if (File.Exists(path)) File.Delete(path); } catch { /* ignore */ }
    }
}
