; Installer of Prism for Windows (ADR 0039), built by Inno Setup 6:
;   scripts\make-installer.ps1
; The script passes the version and the folder of the files to install.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\target\dist\windows"
#endif

[Setup]
AppId={{6F1D2B0A-5C3E-4E7B-9A51-2D8C0F4B7E19}
AppName=Prism
AppVersion={#Version}
AppPublisher=Prism
AppPublisherURL=https://github.com/ouimarcelin/prism
DefaultDirName={autopf}\Prism
DefaultGroupName=Prism
DisableProgramGroupPage=yes
OutputDir={#Source}\..
OutputBaseFilename=Prism-{#Version}-setup
SetupIconFile={#Source}\prism.ico
UninstallDisplayIcon={app}\prism.ico
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile={#Source}\LICENSE-MIT
WizardStyle=modern

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\prism.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\prism.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\NOTICE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Prism"; Filename: "{app}\prism.exe"; IconFilename: "{app}\prism.ico"
Name: "{autodesktop}\Prism"; Filename: "{app}\prism.exe"; IconFilename: "{app}\prism.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\prism.exe"; Description: "{cm:LaunchProgram,Prism}"; Flags: nowait postinstall skipifsilent
