; ScreenForge (Rust portu) — Inno Setup betiği
; Derle: cargo build --release -p sf-app   (bkz. rust/README.md)
; Sonra: iscc rust/setup-rust.iss
;
; Not: sf-app bağımsız bir binary'dir (ONNX/ort yok), ek DLL taşımaz.

#define MyAppName "ScreenForge"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "ScreenForge"
#define MyAppURL "https://github.com/screenforge"
#define MyAppExeName "screenforge-app.exe"

[Setup]
AppId={{C4F1A7D2-9E63-4B18-8A5F-2D7E60B3C9A1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=setup
OutputBaseFilename=ScreenForge-Setup
SetupIconFile=..\ScreenForge\Resources\app.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Tasks]
Name: "desktopicon"; Description: "Masaüstü kısayolu oluştur"; GroupDescription: "Ek simgeler:"
Name: "launchstartup"; Description: "Windows ile birlikte başlat"; GroupDescription: "Sistem:"; Flags: checkedonce

[Files]
; Ana uygulama: tepsi + genel kısayol tuşu. --window ile tam arayüz açılır.
Source: "target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; Opsiyonel CLI (cargo build --release -p sf-cli çalıştırıldıysa dahil edilir).
Source: "target\release\screenforge.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{#MyAppName} Kaldır"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue; Tasks: launchstartup

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{#MyAppName} uygulamasını başlat"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "reg"; Parameters: "delete ""HKCU\Software\Microsoft\Windows\CurrentVersion\Run"" /v ""{#MyAppName}"" /f"; Flags: runhidden; RunOnceId: "RemoveStartupReg"
