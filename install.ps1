param(
  [string]$Version = $env:VELA_VERSION,
  [string]$InstallDir = $(if ($env:VELA_INSTALL_BINDIR) { $env:VELA_INSTALL_BINDIR } else { Join-Path $env:LOCALAPPDATA "Vela\bin" })
)

$ErrorActionPreference = "Stop"
$Repo = "vela-science/vela"
if (-not [Environment]::Is64BitOperatingSystem) {
  throw "Vela publishes a Windows x86-64 bundle only."
}
$Asset = "vela-windows-x86_64.zip"
if (-not $Version) {
  $Version = (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest").tag_name
}
$Base = "https://github.com/$Repo/releases/download/$Version"
$Temp = Join-Path ([IO.Path]::GetTempPath()) ("vela-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Temp | Out-Null

try {
  $Archive = Join-Path $Temp $Asset
  $Checksum = "$Archive.sha256"
  Invoke-WebRequest "$Base/$Asset" -OutFile $Archive
  Invoke-WebRequest "$Base/$Asset.sha256" -OutFile $Checksum
  $Expected = ((Get-Content -Raw $Checksum).Trim() -split '\s+')[0].ToLowerInvariant()
  $Observed = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
  if ($Expected -ne $Observed) {
    throw "Checksum mismatch for $Asset; refusing installation."
  }
  $Unpack = Join-Path $Temp "unpack"
  Expand-Archive $Archive -DestinationPath $Unpack
  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  foreach ($Binary in @("vela.exe", "vela-signer.exe")) {
    $Source = Join-Path $Unpack $Binary
    if (-not (Test-Path $Source -PathType Leaf)) { throw "$Binary is missing from $Asset" }
    Copy-Item -Force $Source (Join-Path $InstallDir $Binary)
  }
  $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if (($UserPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("Path", (($UserPath.TrimEnd(';') + ";" + $InstallDir).TrimStart(';')), "User")
  }
  if (($env:Path -split ';') -notcontains $InstallDir) { $env:Path = "$InstallDir;$env:Path" }
  & (Join-Path $InstallDir "vela.exe") --version
  & (Join-Path $InstallDir "vela-signer.exe") --version
  Write-Host "Installed vela and vela-signer to $InstallDir"
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temp
}
