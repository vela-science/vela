# Release-workflow implementation; not part of the installed product.
param(
  [Parameter(Mandatory = $true)][string]$Archive,
  [Parameter(Mandatory = $true)][string]$ExpectedVersion
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
$AgentWasRunning = $false
$AuthorityKey = Join-Path $Root "authority"
$TrustPinPath = $null

try {
  Expand-Archive $Archive -DestinationPath $Unpack
  $Vela = Join-Path $Unpack "vela.exe"
  if (-not (Test-Path $Vela -PathType Leaf)) { throw "missing release binary: $Vela" }
  if ((& $Vela --version) -ne "vela $ExpectedVersion") { throw "vela version mismatch" }

  # Exercise the current public profile contract from the staged artifact. A
  # version-only smoke cannot detect release bytes built from stale source.
  $Agent = Get-Service ssh-agent -ErrorAction Stop
  $AgentWasRunning = $Agent.Status -eq "Running"
  if (-not $AgentWasRunning) {
    Set-Service ssh-agent -StartupType Manual
    Start-Service ssh-agent
  }
  & ssh-keygen -q -t ed25519 -N '""' -C "Vela release smoke" -f $AuthorityKey
  if ($LASTEXITCODE -ne 0) { throw "failed to generate disposable release-smoke key" }
  & ssh-add $AuthorityKey | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "failed to load disposable release-smoke key" }
  $FingerprintLine = (& ssh-keygen -lf "$AuthorityKey.pub" -E sha256 | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) { throw "failed to fingerprint disposable release-smoke key" }
  $AuthorityFingerprint = ($FingerprintLine -split '\s+')[1]
  if (-not $AuthorityFingerprint.StartsWith("SHA256:")) { throw "invalid release-smoke key fingerprint" }
  $Frontier = Join-Path $Root "frontier"
  & $Vela init $Frontier `
    --name "Release smoke" `
    --scope "Does this bundle read the current Frontier profile?" `
    --key $AuthorityFingerprint `
    --json | Set-Content -Encoding utf8 (Join-Path $Root "init.json")
  if ($LASTEXITCODE -ne 0) { throw "one-command Frontier initialization failed" }
  $Profile = Join-Path $Frontier "frontier.toml"
  if (-not (Test-Path $Profile -PathType Leaf)) { throw "current Frontier profile missing" }
  if (Test-Path (Join-Path $Frontier "frontier.yaml")) { throw "retired Frontier profile emitted" }
  $Initialized = (Get-Content -Raw (Join-Path $Root "init.json")) | ConvertFrom-Json
  $TrustPinPath = $Initialized.authority.local_trust.anchor_path
  $Status = (& $Vela status $Frontier --json | Out-String) | ConvertFrom-Json
  if ($Initialized.schema -ne "vela.frontier-init.v3") { throw "current init schema mismatch" }
  if ($Initialized.authority.state -ne "initialized") { throw "Frontier authority was not initialized" }
  if ($Initialized.scientific_object_count -ne 0) { throw "new Frontier contains scientific objects" }
  if (-not $Initialized.repository.repository_root.StartsWith("sha256:")) { throw "repository root missing" }
  if (-not $Initialized.next_action.StartsWith("vela submit ")) { throw "current next action mismatch" }
  if ($Status.schema -ne "vela.status.v3") { throw "current status schema mismatch" }
  if ($Status.integrity.replay -ne "verified") { throw "replay was not verified" }
  if ($Status.integrity.strict -ne "pass") { throw "strict integrity did not pass" }
  if ($Status.integrity.blocker_count -ne 0) { throw "new Frontier has integrity blockers" }
  if ($Status.counts.claims -ne 0) { throw "new Frontier contains Claims" }
  if ($Status.actions.work.mode -ne "direct_submission") { throw "current work action mismatch" }

  foreach ($Binary in @("vela.exe")) {
    Copy-Item -Force (Join-Path $Unpack $Binary) (Join-Path $Bin $Binary)
  }
  & (Join-Path $Bin "vela.exe") --version

  foreach ($Binary in @("vela.exe")) {
    $Installed = Join-Path $Bin $Binary
    Copy-Item -Force (Join-Path $Unpack $Binary) $Installed
    Remove-Item -Force $Installed
    if (Test-Path $Installed) { throw "uninstall left product byte: $Installed" }
  }
} finally {
  if ($TrustPinPath -and $TrustPinPath -match '[\\/]\.vela[\\/]trust[\\/]authorities[\\/]vfr_[^\\/]+\.json$') {
    Remove-Item -Force -ErrorAction SilentlyContinue $TrustPinPath
  }
  if (Test-Path $AuthorityKey -PathType Leaf) {
    & ssh-add -d $AuthorityKey 2>&1 | Out-Null
  }
  if (-not $AgentWasRunning) {
    Stop-Service ssh-agent -ErrorAction SilentlyContinue
  }
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Root
}

Write-Host "release bundle smoke passed: $([IO.Path]::GetFileName($Archive)) ($ExpectedVersion)"
