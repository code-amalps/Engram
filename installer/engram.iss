#define MyAppName "Engram"
#define MyAppVersion "1.5.3"
#define MyAppPublisher "Engram Team"
#define MyAppURL "https://github.com/code-amalps/Engram"
#define MyAppExeName "engram.exe"

[Setup]
AppId={{9F57C859-9943-4C3D-B910-8B39B7C4155A}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\Engram
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
InfoBeforeFile=WELCOME.txt
LicenseFile=PRIVACY_NOTICE.txt
OutputDir=..\dist\installer
OutputBaseFilename=EngramSetup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline
CloseApplications=yes
RestartApplications=no
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\target\release\engram.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "PRIVACY_NOTICE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "WELCOME.txt"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "addtopath"; Description: "Add Engram to User PATH environment variable (enables 'engram' command everywhere)"; Flags: checkedonce

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{code:GetNewPath}"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

[Run]
Filename: "{app}\bin\engram.exe"; Parameters: "--version"; Flags: runhidden postinstall nowait skipifsilent; Description: "Verify installation"

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + UpperCase(Param) + ';', ';' + UpperCase(OrigPath) + ';') = 0;
  if Result then
    Result := Pos(';' + UpperCase(Param) + '\;', ';' + UpperCase(OrigPath) + ';') = 0;
end;

function GetNewPath(Param: string): string;
var
  OrigPath: string;
  NewDir: string;
begin
  NewDir := ExpandConstant('{app}\bin');
  if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    if Length(OrigPath) > 0 then
    begin
      if OrigPath[Length(OrigPath)] = ';' then
        Result := OrigPath + NewDir
      else
        Result := OrigPath + ';' + NewDir;
    end
    else
      Result := NewDir;
  end
  else
    Result := NewDir;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  AppDir: string;
  OrigPath: string;
  P: Integer;
begin
  if CurUninstallStep = usUninstall then
  begin
    AppDir := ExpandConstant('{app}\bin');
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
    begin
      P := Pos(';' + UpperCase(AppDir) + ';', ';' + UpperCase(OrigPath) + ';');
      if P > 0 then
      begin
        OrigPath := ';' + OrigPath + ';';
        StringChangeEx(OrigPath, ';' + AppDir + ';', ';', True);
        StringChangeEx(OrigPath, ';' + AppDir + '\;', ';', True);
        if Length(OrigPath) > 1 then
        begin
          if OrigPath[1] = ';' then Delete(OrigPath, 1, 1);
          if (Length(OrigPath) > 0) and (OrigPath[Length(OrigPath)] = ';') then
            Delete(OrigPath, Length(OrigPath), 1);
        end;
        RegWriteStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath);
      end;
    end;
  end;
end;
