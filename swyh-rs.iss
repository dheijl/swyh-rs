; swyh-rs.iss

[Setup]
AppName=swyh-rs
AppVersion=1.7.0-beta
WizardStyle=modern
DefaultDirName={autopf}\swyh-rs
DefaultGroupName=swyh-rs
UninstallDisplayIcon={app}\swyh-rs.exe
Compression=lzma2
SolidCompression=yes
SourceDir=C:\Users\Danny\source\rust\projects\swyh-rs
OutputDir=Output
OutputBaseFilename=Setup-swyh-rs
; "ArchitecturesAllowed=x64" specifies that Setup cannot run on
; anything but x64.
ArchitecturesAllowed=x64
; "ArchitecturesInstallIn64BitMode=x64" requests that the install be
; done in "64-bit mode" on x64, meaning it should use the native
; 64-bit Program Files directory and the 64-bit view of the registry.
ArchitecturesInstallIn64BitMode=x64
AppPublisher=Danny Heijl
AppPublisherURL=https://github.com/dheijl/swyh-rs/
AppSupportURL=https://github.com/dheijl/swyh-rs/issues
AppUpdatesURL=https://github.com/dheijl/swyh-rs/releases
AppComments=Stream What You Hear written in Rust
AppReadmeFile=Readme.md

[Files]
Source: "target\release\swyh-rs.exe"; DestDir: "{app}"; DestName: "swyh-rs.exe"
Source: "Readme.md"; DestDir: "{app}"; Flags: isreadme

[Icons]
Name: "{group}\swyh-rs"; Filename: "{app}\swyh-rs.exe"
