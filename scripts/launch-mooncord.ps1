param(
    [string]$ClientDirectory = 'C:\Users\Tropical\Downloads\DiscordPhoenixSetup.exe (1)\recovered-mooncord',
    [string]$LogFile = (Join-Path $env:TEMP 'erebus-mooncord.log')
)
$ErrorActionPreference = 'Stop'
$client = Join-Path $ClientDirectory 'Discord-Mooncord.exe'
foreach ($file in @($client, (Join-Path $ClientDirectory 'd3d11.dll'), (Join-Path $ClientDirectory 'd3d11_orig.dll'))) {
    if (-not (Test-Path -LiteralPath $file)) { throw "Missing required file: $file" }
}
$env:EREBUS_LOG_FILE = $LogFile
$env:EREBUS_LOG = 'info'
Start-Process -FilePath $client -WorkingDirectory $ClientDirectory
Write-Output "Erebus log: $LogFile"
