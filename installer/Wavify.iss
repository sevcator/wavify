#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef BuildDir
  #define BuildDir "..\target\x86_64-pc-windows-gnu\release"
#endif

[Setup]
AppId={{7D753A73-8F0E-4E8F-9507-8FAFE77F1B4D}
AppName=Wavify
AppVersion={#AppVersion}
AppPublisher=Wavify
DefaultDirName={localappdata}\Programs\Wavify
DefaultGroupName=Wavify
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64
OutputDir=..\dist
OutputBaseFilename=Wavify-Setup
SetupIconFile=..\assets\icon\wavify.ico
UninstallDisplayIcon={app}\wavify.exe
CloseApplications=yes
CloseApplicationsFilter=wavify.exe
RestartApplications=no
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "{#BuildDir}\wavify.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BuildDir}\WebView2Loader.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\MicrosoftEdgeWebview2Setup.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{autoprograms}\Wavify"; Filename: "{app}\wavify.exe"
Name: "{autodesktop}\Wavify"; Filename: "{app}\wavify.exe"; Tasks: desktopicon

[Run]
Filename: "{tmp}\MicrosoftEdgeWebview2Setup.exe"; Parameters: "/silent /install"; StatusMsg: "Installing Microsoft Edge WebView2 Runtime..."; Flags: waituntilterminated
Filename: "{app}\wavify.exe"; Description: "Launch Wavify"; Flags: postinstall nowait skipifsilent
