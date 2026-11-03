$p = Start-Process -FilePath "target\release\screenforge.exe" -ArgumentList "png","$env:TEMP\sf-out\one.png" -PassThru -NoNewWindow
$peak = 0
while (-not $p.HasExited) {
  try { $p.Refresh(); if ($p.WorkingSet64 -gt $peak) { $peak = $p.WorkingSet64 } } catch {}
  Start-Sleep -Milliseconds 20
}
"Single capture peak: {0:N1} MB" -f ($peak/1MB)
