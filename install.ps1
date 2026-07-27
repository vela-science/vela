param(
  [string]$Version = $env:VELA_VERSION,
  [string]$InstallDir = $(if ($env:VELA_INSTALL_BINDIR) { $env:VELA_INSTALL_BINDIR } else { Join-Path $env:LOCALAPPDATA "Vela\bin" }),
  [ValidateSet("Install", "Upgrade", "Uninstall")][string]$Action = "Install"
)

$ErrorActionPreference = "Stop"
$Repo = "vela-science/vela"
if ($Action -eq "Uninstall") {
  foreach ($Binary in @("vela.exe")) {
    Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $InstallDir $Binary)
  }
  Write-Host "Removed Vela. Frontier data was preserved."
  exit 0
}
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
  $Trust = "$Archive.trust.json"
  $TrustChecksum = "$Trust.sha256"
  Invoke-WebRequest "$Base/$Asset" -OutFile $Archive
  Invoke-WebRequest "$Base/$Asset.sha256" -OutFile $Checksum
  Invoke-WebRequest "$Base/$Asset.trust.json" -OutFile $Trust
  Invoke-WebRequest "$Base/$Asset.trust.json.sha256" -OutFile $TrustChecksum
  $Expected = ((Get-Content -Raw $Checksum).Trim() -split '\s+')[0].ToLowerInvariant()
  $Observed = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
  if ($Expected -ne $Observed) {
    throw "Checksum mismatch for $Asset; refusing installation."
  }
  if ($env:VELA_EXPECTED_SHA256 -and $Observed -ne $env:VELA_EXPECTED_SHA256.ToLowerInvariant()) {
    throw "$Asset differs from the ecosystem-lock SHA-256; refusing installation."
  }
  $ExpectedTrust = ((Get-Content -Raw $TrustChecksum).Trim() -split '\s+')[0].ToLowerInvariant()
  $ObservedTrust = (Get-FileHash -Algorithm SHA256 $Trust).Hash.ToLowerInvariant()
  if ($ExpectedTrust -ne $ObservedTrust) {
    throw "Checksum mismatch for $Asset trust metadata; refusing installation."
  }
  if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "GitHub CLI is required to verify build provenance: https://cli.github.com/"
  }
  & gh attestation verify $Archive --repo $Repo --signer-workflow "$Repo/.github/workflows/release.yml" --source-ref "refs/tags/$Version" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "GitHub build provenance verification failed for $Asset" }
  & gh attestation verify $Trust --repo $Repo --signer-workflow "$Repo/.github/workflows/release.yml" --source-ref "refs/tags/$Version" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "GitHub build provenance verification failed for $Asset trust metadata" }
  $TrustRecord = Get-Content -Raw $Trust | ConvertFrom-Json
  if ($TrustRecord.schema -ne "vela.release-trust.v1" -or
      $TrustRecord.artifact -ne $Asset -or
      $TrustRecord.artifact_class -ne "portable") {
    throw "Invalid release trust metadata for $Asset"
  }
  $Unpack = Join-Path $Temp "unpack"
  Expand-Archive $Archive -DestinationPath $Unpack
  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  foreach ($Binary in @("vela.exe")) {
    $Source = Join-Path $Unpack $Binary
    if (-not (Test-Path $Source -PathType Leaf)) { throw "$Binary is missing from $Asset" }
    if ($TrustRecord.platform_signature -eq "authenticode") {
      $Signature = Get-AuthenticodeSignature $Source
      if ($Signature.Status -ne "Valid") {
        throw "$Binary has an invalid Authenticode signature: $($Signature.Status)"
      }
    } elseif ($TrustRecord.platform_signature -ne "absent") {
      throw "Unsupported Windows platform-signature tier: $($TrustRecord.platform_signature)"
    }
    Copy-Item -Force $Source (Join-Path $InstallDir $Binary)
  }
  $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if (($UserPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("Path", (($UserPath.TrimEnd(';') + ";" + $InstallDir).TrimStart(';')), "User")
  }
  if (($env:Path -split ';') -notcontains $InstallDir) { $env:Path = "$InstallDir;$env:Path" }
  & (Join-Path $InstallDir "vela.exe") --version
  Write-Host "Installed vela to $InstallDir"
  if ($TrustRecord.platform_signature -eq "absent") {
    Write-Host "Note: this is a GitHub-attested portable build without an Authenticode signature."
  }
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temp
}
