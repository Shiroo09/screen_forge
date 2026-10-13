using Microsoft.Win32;

namespace ScreenForge.Background;

/// <summary>
/// Görüntü dosyalarının klasik Explorer menüsüne "Arka planını kaldır" ekler.
/// Windows 11 sade menüsü imzasız paket kimliği olmadan bu öğeyi göstermez;
/// öğe "Daha fazla seçenek göster" altında durur.
/// </summary>
public static class ShellBackgroundMenu
{
    public const string Verb = "ScreenForge.RemoveBackground";
    public const string Caption = "Arka planını kaldır";

    private static readonly string[] Extensions =
    [
        ".png", ".jpg", ".jpeg", ".jfif", ".bmp", ".gif", ".tif", ".tiff", ".webp", ".heic", ".heif", ".avif", ".ico",
    ];

    public static void Register()
    {
        var exe = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(exe))
            return;

        string command = $"\"{exe}\" {BackgroundFiles.Argument} \"%1\"";
        try
        {
            WriteVerb(@"Software\Classes\SystemFileAssociations\image\shell", command, exe);
            foreach (var extension in Extensions)
                WriteVerb($@"Software\Classes\SystemFileAssociations\{extension}\shell", command, exe);
        }
        catch
        {
            // Menü yazılamazsa düzenleyici yine tepsi ve komut satırından açılır.
        }
    }

    private static void WriteVerb(string shellKey, string command, string exe)
    {
        using var verb = Registry.CurrentUser.CreateSubKey(shellKey + "\\" + Verb);
        verb?.SetValue("", Caption);
        verb?.SetValue("MUIVerb", Caption);
        verb?.SetValue("Icon", exe + ",0");
        verb?.SetValue("NeverDefault", "");
        using var cmd = verb?.CreateSubKey("command");
        cmd?.SetValue("", command);
    }
}
