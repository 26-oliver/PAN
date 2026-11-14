$ErrorActionPreference = 'Stop'
$appDir = Join-Path $env:LOCALAPPDATA 'Programs\PAN'
$exe = Join-Path $appDir 'pan.exe'
$icon = Join-Path $appDir 'pan.ico'
New-Item -ItemType Directory -Force $appDir | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'pan.exe') $exe -Force
Copy-Item (Join-Path $PSScriptRoot 'pan.ico') $icon -Force

function Set-RegistryValue($Key, $Name, $Value) {
    $null = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($Key)
    $subkey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Key, $true)
    try { $subkey.SetValue($Name, $Value) } finally { $subkey.Dispose() }
}

$extensions = 'mp4 m4v mov mkv webm avi wmv mpg mpeg ts m2ts flv vob ogv 3gp asf mp3 m4a wav flac ogg opus aac aiff alac wma ac3'.Split(' ')
$command = '"' + $exe + '" "%1"'
$appKey = 'Software\Classes\Applications\pan.exe'
$progId = 'Software\Classes\PAN.Media'
Set-RegistryValue $appKey 'FriendlyAppName' 'PAN'
Set-RegistryValue "$appKey\shell\open\command" '' $command
Set-RegistryValue $progId '' 'PAN media file'
Set-RegistryValue "$progId\DefaultIcon" '' ('"' + $icon + '",0')
Set-RegistryValue "$progId\shell\open\command" '' $command
Set-RegistryValue 'Software\RegisteredApplications' 'PAN' 'Software\PAN\Capabilities'
Set-RegistryValue 'Software\PAN\Capabilities' 'ApplicationName' 'PAN'
Set-RegistryValue 'Software\PAN\Capabilities' 'ApplicationDescription' 'Play Anything Now'
foreach ($extension in $extensions) {
    Set-RegistryValue "$appKey\SupportedTypes" ".$extension" ''
    Set-RegistryValue "Software\Classes\.$extension\OpenWithProgids" 'PAN.Media' ''
    Set-RegistryValue 'Software\PAN\Capabilities\FileAssociations' ".$extension" 'PAN.Media'
}

$shell = New-Object -ComObject WScript.Shell
$programs = [Environment]::GetFolderPath('Programs')
$shortcut = $shell.CreateShortcut((Join-Path $programs 'PAN.lnk'))
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $appDir
$shortcut.IconLocation = "$icon,0"
$shortcut.Save()

Write-Host 'PAN installed. In File Explorer, choose Open with > PAN.'
