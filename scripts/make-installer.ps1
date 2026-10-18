# Builds the Windows installer of Prism (Prism-<version>-setup.exe), on a
# Windows PC with Rust, the Visual Studio Build Tools and Inno Setup 6
# (ADR 0039).
#
#   powershell -ExecutionPolicy Bypass -File scripts\make-installer.ps1 [-Ndi]
#
# Signing, when the variables are set (signtool of the Windows SDK):
#   PRISM_SIGN_CERT       path of a .pfx code signing certificate
#   PRISM_SIGN_PASSWORD   its password
param([switch]$Ndi)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$features = @()
if ($Ndi) { $features = @("--features", "ndi") }
cargo build --release -p prism-app @features
if ($LASTEXITCODE -ne 0) { throw "cargo build a échoué" }

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$dist = "target\dist\windows"
New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item target\release\prism.exe $dist
Copy-Item LICENSE-MIT, LICENSE-APACHE, NOTICE $dist

# Icon: the logo at 256 pixels, stored as PNG inside an .ico file (read by
# Windows Vista and later).
Add-Type -AssemblyName System.Drawing
$logo = [System.Drawing.Image]::FromFile((Resolve-Path assets\icon-1024.png))
$small = New-Object System.Drawing.Bitmap 256, 256
$graphics = [System.Drawing.Graphics]::FromImage($small)
$graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$graphics.DrawImage($logo, 0, 0, 256, 256)
$png = New-Object System.IO.MemoryStream
$small.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose(); $small.Dispose(); $logo.Dispose()
$bytes = $png.ToArray()
$ico = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $ico
$writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]1)
$writer.Write([Byte]0); $writer.Write([Byte]0); $writer.Write([Byte]0); $writer.Write([Byte]0)
$writer.Write([UInt16]1); $writer.Write([UInt16]32)
$writer.Write([UInt32]$bytes.Length); $writer.Write([UInt32]22)
$writer.Write($bytes)
[System.IO.File]::WriteAllBytes((Join-Path (Resolve-Path $dist) "prism.ico"), $ico.ToArray())

function Sign($path) {
    if ($env:PRISM_SIGN_CERT) {
        signtool sign /f $env:PRISM_SIGN_CERT /p $env:PRISM_SIGN_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $path
        if ($LASTEXITCODE -ne 0) { throw "signature de $path impossible" }
    }
}
Sign "$dist\prism.exe"

$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) { throw "Inno Setup 6 introuvable : $iscc" }
& $iscc "/DVersion=$version" "/DSource=$(Resolve-Path $dist)" installer\windows\prism.iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup a échoué" }
$setup = "target\dist\Prism-$version-setup.exe"
Sign $setup
if (-not $env:PRISM_SIGN_CERT) { Write-Warning "Installeur non signé : définir PRISM_SIGN_CERT. SmartScreen avertira à l'ouverture." }
Write-Output (Resolve-Path $setup)
