$exe = "target/release/lumen.exe"
$mb = (Get-Item $exe).Length / 1MB
Write-Host ("exe: {0:N1} MB" -f $mb)
if ($mb -gt 15) { throw "exe supera 15 MB" }

function Measure-Run {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process $exe -PassThru
    while ($p.MainWindowHandle -eq 0 -and $sw.ElapsedMilliseconds -lt 5000) { Start-Sleep -Milliseconds 5; $p.Refresh() }
    $ms = $sw.ElapsedMilliseconds
    Start-Sleep -Seconds 3; $p.Refresh()
    $ram = $p.PrivateMemorySize64 / 1MB   # privada: el working set suma DLLs compartidas de Windows
    $p.Kill()
    [pscustomobject]@{ ms = $ms; ram = $ram }
}

Measure-Run | Out-Null   # calentamiento: la primera corrida paga el escaneo del antivirus
$r = Measure-Run
Write-Host ("arranque: {0} ms, RAM: {1:N1} MB" -f $r.ms, $r.ram)
if ($r.ms -gt 300) { throw "arranque > 300 ms" }
if ($r.ram -gt 20) { throw "RAM > 20 MB" }
