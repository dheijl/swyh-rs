; swyh-rs.iss

#include "CodeDependencies.iss"

[Setup]
AppName=swyh-rs
AppVersion=1.11.1
WizardStyle=modern
DefaultDirName={autopf}\swyh-rs
DefaultGroupName=swyh-rs
UninstallDisplayIcon={app}\swyh-rs.exe
Compression=lzma2
SolidCompression=yes
SourceDir=C:\Users\danny\Documents\Development\GitHub\swyh-rs
OutputDir=Output
OutputBaseFilename=Setup-swyh-rs
; "ArchitecturesAllowed=x64" specifies that Setup cannot run on
; anything but x64.
ArchitecturesAllowed=x64compatible
; "ArchitecturesInstallIn64BitMode=x64" requests that the install be
; done in "64-bit mode" on x64, meaning it should use the native
; 64-bit Program Files directory and the 64-bit view of the registry.
ArchitecturesInstallIn64BitMode=x64compatible
AppPublisher=Danny Heijl
AppPublisherURL=https://github.com/dheijl/swyh-rs/
AppSupportURL=https://github.com/dheijl/swyh-rs/issues
AppUpdatesURL=https://github.com/dheijl/swyh-rs/releases
AppComments=Stream What You Hear written in Rust
AppReadmeFile=Readme.md

[Files]
Source: "target\release\swyh-rs.exe"; DestDir: "{app}"; DestName: "swyh-rs.exe"
Source: "target\release\swyh-rs-cli.exe"; DestDir: "{app}"; DestName: "swyh-rs-cli.exe"
Source: "Readme.md"; DestDir: "{app}"; Flags: isreadme

[Icons]
Name: "{group}\swyh-rs"; Filename: "{app}\swyh-rs.exe"

[Code]
function InitializeSetup: Boolean;
begin
  // Rust binaries compiled with the msvc toolchain need VC runtime
  Dependency_AddVC2015To2022;
  Result := True;
end;
