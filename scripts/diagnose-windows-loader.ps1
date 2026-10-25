param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$Binary = (Resolve-Path $Binary).Path
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class LoaderProbe {
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, ExactSpelling=true, SetLastError=true)] public static extern IntPtr LoadLibraryW(string path);
  [DllImport("kernel32.dll", CharSet=CharSet.Ansi, ExactSpelling=true, SetLastError=true)] public static extern IntPtr GetProcAddress(IntPtr module, string name);
  [DllImport("kernel32.dll", EntryPoint="GetProcAddress", ExactSpelling=true, SetLastError=true)] public static extern IntPtr GetOrdinal(IntPtr module, IntPtr ordinal);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, ExactSpelling=true, SetLastError=true)] public static extern bool SetDllDirectoryW(string path);
}
'@
$directory = Split-Path $Binary
[void][LoaderProbe]::SetDllDirectoryW($directory)
try {
  $imports = & node "$PSScriptRoot/windows-pe-imports.mjs" $Binary | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect PE imports' }
  foreach ($entry in $imports) {
    $library = $entry.dll
    foreach ($folder in @($directory, [Environment]::SystemDirectory) + ($env:PATH -split ';')) {
      if (!$folder) { continue }
      $candidate = Join-Path $folder $entry.dll
      if (Test-Path -LiteralPath $candidate -PathType Leaf) { $library = $candidate; break }
    }
    $module = [LoaderProbe]::LoadLibraryW($library)
    if ($module -eq [IntPtr]::Zero) {
      Write-Output "LOAD FAILED $library Win32=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
      continue
    }
    $missing = @()
    foreach ($symbol in $entry.symbols) {
      $address = if ($symbol -is [string]) { [LoaderProbe]::GetProcAddress($module, $symbol) } else { [LoaderProbe]::GetOrdinal($module, [IntPtr][int]$symbol) }
      if ($address -eq [IntPtr]::Zero) { $missing += $symbol }
    }
    if ($missing.Count) { Write-Output "MISSING EXPORTS $library : $($missing -join ', ')" }
    else { Write-Output "OK $library ($($entry.symbols.Count) imports)" }
  }
} finally {
  [void][LoaderProbe]::SetDllDirectoryW($null)
}
