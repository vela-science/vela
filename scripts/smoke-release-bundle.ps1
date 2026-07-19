param(
  [Parameter(Mandatory = $true)][string]$Archive,
  [Parameter(Mandatory = $true)][string]$ExpectedVersion,
  [Parameter(Mandatory = $true)][bool]$RequirePlatformSignature
)

$ErrorActionPreference = "Stop"
$Checksum = "$Archive.sha256"
$Sbom = "$Archive.spdx.json"
$SbomChecksum = "$Sbom.sha256"
foreach ($Path in @($Archive, $Checksum, $Sbom, $SbomChecksum)) {
  if (-not (Test-Path $Path -PathType Leaf)) { throw "missing release bundle file: $Path" }
}

function Assert-Checksum([string]$Path, [string]$ChecksumPath) {
  $Expected = ((Get-Content -Raw $ChecksumPath).Trim() -split '\s+')[0].ToLowerInvariant()
  $Observed = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
  if ($Expected -ne $Observed) { throw "checksum mismatch for $Path" }
}

Assert-Checksum $Archive $Checksum
Assert-Checksum $Sbom $SbomChecksum
$Root = Join-Path ([IO.Path]::GetTempPath()) ("vela-release-smoke-" + [Guid]::NewGuid())
$Unpack = Join-Path $Root "unpack"
$Prefix = Join-Path $Root "prefix"
$Bin = Join-Path $Prefix "bin"
New-Item -ItemType Directory -Force $Unpack, $Bin | Out-Null

try {
  Expand-Archive $Archive -DestinationPath $Unpack
  $Vela = Join-Path $Unpack "vela.exe"
  $Signer = Join-Path $Unpack "vela-signer.exe"
  foreach ($Binary in @($Vela, $Signer)) {
    if (-not (Test-Path $Binary -PathType Leaf)) { throw "missing release binary: $Binary" }
    if ($RequirePlatformSignature) {
      $Signature = Get-AuthenticodeSignature $Binary
      if ($Signature.Status -ne "Valid") { throw "invalid Authenticode signature for $Binary`: $($Signature.Status)" }
    }
  }
  if ((& $Vela --version) -ne "vela $ExpectedVersion") { throw "vela version mismatch" }
  if ((& $Signer --version) -ne "vela-signer $ExpectedVersion") { throw "vela-signer version mismatch" }

  foreach ($Binary in @("vela.exe", "vela-signer.exe")) {
    Copy-Item -Force (Join-Path $Unpack $Binary) (Join-Path $Bin $Binary)
  }
  & (Join-Path $Bin "vela.exe") --version
  & (Join-Path $Bin "vela-signer.exe") --version

  foreach ($Binary in @("vela.exe", "vela-signer.exe")) {
    $Installed = Join-Path $Bin $Binary
    Copy-Item -Force (Join-Path $Unpack $Binary) $Installed
    Remove-Item -Force $Installed
    if (Test-Path $Installed) { throw "uninstall left product byte: $Installed" }
  }
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Root
}

Write-Host "release bundle smoke passed: $([IO.Path]::GetFileName($Archive)) ($ExpectedVersion)"
