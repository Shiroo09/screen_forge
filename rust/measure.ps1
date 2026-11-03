$p = Start-Process -FilePath "target\release\screenforge.exe" -ArgumentList "gif","$env:TEMP\sf-out\mem.gif","8","80" -PassThru -NoNewWindow
$peak = 0
while (-not $p.HasExited) {
  try { $p.Refresh(); if ($p.WorkingSet64 -gt $peak) { $peak = $p.WorkingSet64 } } catch {}
  Start-Sleep -Milliseconds 30
}
"Rust CLI peak working set: {0:N1} MB" -f ($peak/1MB)
