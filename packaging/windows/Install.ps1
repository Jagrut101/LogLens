$ErrorActionPreference = 'Stop'
try {
    $destination = Join-Path $env:LOCALAPPDATA 'Programs\LogLens'
    $exe = Join-Path $destination 'LogLens.exe'
    if ([IO.Path]::GetFullPath($PSScriptRoot).TrimEnd('\') -eq [IO.Path]::GetFullPath($destination).TrimEnd('\')) {
        throw 'Already in the installation folder. Run LogLens.exe directly.'
    }
    foreach ($process in @(Get-Process -Name LogLens -ErrorAction SilentlyContinue)) {
        if ($process.Path -eq $exe) { throw 'Close the installed LogLens before updating it.' }
    }
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    foreach ($item in Get-ChildItem -LiteralPath $PSScriptRoot) {
        if ($item.Name -notin @('Install.cmd','Install.ps1')) {
            Copy-Item -LiteralPath $item.FullName -Destination $destination -Recurse -Force
        }
    }
    $shell = New-Object -ComObject WScript.Shell
    foreach ($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))) {
        if ($folder) {
            $shortcut = $shell.CreateShortcut((Join-Path $folder 'LogLens.lnk'))
            $shortcut.TargetPath = $exe
            $shortcut.WorkingDirectory = $destination
            $shortcut.IconLocation = "$exe,0"
            $shortcut.Save()
        }
    }
    Write-Host "Installed to $destination"
    exit 0
} catch {
    Write-Host "Installation failed: $($_.Exception.Message)"
    exit 1
}
