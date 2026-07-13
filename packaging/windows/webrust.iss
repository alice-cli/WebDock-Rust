; Inno Setup script — builds WebRust-Setup-{version}.exe
; Compile from repo root after release build:
;   iscc packaging/windows/webrust.iss /DMyAppVersion=0.1.1 /DMyAppSource=dist\webrust

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#ifndef MyAppSource
  #define MyAppSource "dist\webrust"
#endif

[Setup]
AppId={{A8F3C2E1-9B47-4D6A-8E2F-1C0B5A7D9E33}
AppName=WebRust
AppVersion={#MyAppVersion}
AppPublisher=WebDock / alice-cli
AppPublisherURL=https://github.com/alice-cli/WebDock-Rust
AppSupportURL=https://github.com/alice-cli/WebDock-Rust/issues
DefaultDirName={autopf}\WebRust
DefaultGroupName=WebRust
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=WebRust-Setup-{#MyAppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\WebRust.exe
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startmenu"; Description: "Create Start Menu shortcut"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce

[Files]
Source: "{#MyAppSource}\WebRust.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyAppSource}\webui\*"; DestDir: "{app}\webui"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#MyAppSource}\README.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#MyAppSource}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\WebRust"; Filename: "{app}\WebRust.exe"; WorkingDir: "{app}"
Name: "{group}\Uninstall WebRust"; Filename: "{uninstallexe}"
Name: "{autodesktop}\WebRust"; Filename: "{app}\WebRust.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\WebRust.exe"; Description: "Launch WebRust"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}\webui"
